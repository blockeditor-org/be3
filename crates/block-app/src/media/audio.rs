use std::ffi::CString;
use std::io::Cursor;
use std::net::Shutdown;
use std::os::unix::net::UnixStream;
use std::sync::Arc;

use async_io::Async;
use futures_util::future::{Either, select};
use futures_util::io::{AsyncReadExt, AsyncWriteExt};
use futures_util::pin_mut;
use pulseaudio::protocol::{
    self, AuthParams, AuthReply, ChannelVolume, Command, CommandReply, GetSinkInfo, GetSourceInfo,
    Prop, Props, PulseError, SetClientNameReply, SetDeviceMuteParams, SetDeviceVolumeParams,
    SinkInfo, SourceInfo, SubscriptionMask, Volume,
};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender, unbounded_channel};

use block_plugin_api::MediaLevel;

use super::{Audio, AudioRequest, MediaEvent};
use crate::host::WakingSender;

const LONGEST_FRAME: usize = 16 << 20;

pub(crate) struct PulseAudio {
    requests: UnboundedSender<AudioRequest>,
}

impl PulseAudio {
    pub(crate) fn start(events: WakingSender<MediaEvent>) -> Self {
        let (requests, requested) = unbounded_channel();
        crate::dbus::spawn(run(requested, events));
        Self { requests }
    }
}

impl Audio for PulseAudio {
    fn request(&self, request: AudioRequest) {
        let _ = self.requests.send(request);
    }
}

pub(super) fn stepped(channels: &[u32], by: f32) -> Vec<u32> {
    let norm = Volume::NORM.as_u32();
    let loudest = channels.iter().copied().max().unwrap_or_default();
    if by > 0.0 && loudest >= norm {
        return channels.to_vec();
    }
    let step = (by.abs() * norm as f32).round() as u32;
    let target = match by > 0.0 {
        true => loudest.saturating_add(step).min(norm),
        false => loudest.saturating_sub(step),
    };
    scaled(channels, target)
}

pub(super) fn set(channels: &[u32], level: f32) -> Vec<u32> {
    let norm = Volume::NORM.as_u32();
    scaled(
        channels,
        (level.clamp(0.0, 1.0) * norm as f32).round() as u32,
    )
}

fn scaled(channels: &[u32], target: u32) -> Vec<u32> {
    let loudest = channels.iter().copied().max().unwrap_or_default();
    match loudest {
        0 => vec![target; channels.len()],
        _ => channels
            .iter()
            .map(|&channel| (u64::from(channel) * u64::from(target) / u64::from(loudest)) as u32)
            .collect(),
    }
}

pub(super) fn level(channels: &[u32], muted: bool) -> MediaLevel {
    let loudest = channels.iter().copied().max().unwrap_or_default();
    MediaLevel {
        level: loudest as f32 / Volume::NORM.as_u32() as f32,
        muted,
    }
}

async fn run(mut requested: UnboundedReceiver<AudioRequest>, events: WakingSender<MediaEvent>) {
    let mut pending = None;
    let mut reconnect = true;
    loop {
        match Connection::open().await {
            Ok(mut connection) => match connection.serve(&mut requested, &events, pending).await {
                Ok(()) => return,
                Err(error) => {
                    eprintln!("block-app: the PulseAudio connection ended: {error}");
                    pending = None;
                    if std::mem::take(&mut reconnect) {
                        continue;
                    }
                }
            },
            Err(error) => eprintln!("block-app: PulseAudio is not reachable: {error}"),
        }
        let _ = events.send(MediaEvent::Audio {
            output: None,
            input: None,
        });
        match requested.recv().await {
            Some(request) => {
                pending = Some(request);
                reconnect = true;
            }
            None => return,
        }
    }
}

enum Incoming {
    Reply(u32),
    Error(u32, PulseError),
    Event,
    Other,
}

struct Connection {
    stream: Arc<Async<UnixStream>>,
    frames: UnboundedReceiver<Vec<u8>>,
    version: u16,
    sequence: u32,
    changed: bool,
}

impl Drop for Connection {
    fn drop(&mut self) {
        let _ = self.stream.get_ref().shutdown(Shutdown::Both);
    }
}

impl Connection {
    async fn open() -> Result<Self, String> {
        let path = pulseaudio::socket_path_from_env().ok_or("there is no server socket")?;
        let cookie = pulseaudio::cookie_path_from_env()
            .and_then(|path| std::fs::read(path).ok())
            .unwrap_or_default();
        let stream = Arc::new(
            Async::<UnixStream>::connect(&path)
                .await
                .map_err(|error| format!("{}: {error}", path.display()))?,
        );
        let (sender, frames) = unbounded_channel();
        crate::dbus::spawn(read_frames(Arc::clone(&stream), sender));
        let mut connection = Self {
            stream,
            frames,
            version: protocol::MAX_VERSION,
            sequence: 0,
            changed: false,
        };
        let auth: AuthReply = connection
            .reply(Command::Auth(AuthParams {
                version: protocol::MAX_VERSION,
                supports_shm: false,
                supports_memfd: false,
                cookie,
            }))
            .await?
            .map_err(|error| format!("the server refused the connection: {error:?}"))?;
        connection.version = auth.version.min(protocol::MAX_VERSION);
        let mut props = Props::new();
        props.set(Prop::ApplicationName, c"Block");
        let _: SetClientNameReply = connection
            .reply(Command::SetClientName(props))
            .await?
            .map_err(|error| format!("the server refused the client's name: {error:?}"))?;
        connection
            .acknowledged(Command::Subscribe(
                SubscriptionMask::SINK | SubscriptionMask::SOURCE | SubscriptionMask::SERVER,
            ))
            .await?
            .map_err(|error| format!("the server refused a subscription: {error:?}"))?;
        Ok(connection)
    }

    async fn serve(
        &mut self,
        requested: &mut UnboundedReceiver<AudioRequest>,
        events: &WakingSender<MediaEvent>,
        pending: Option<AudioRequest>,
    ) -> Result<(), String> {
        self.publish(events).await?;
        if let Some(request) = pending {
            self.handle(request).await?;
        }
        loop {
            if std::mem::take(&mut self.changed) {
                self.publish(events).await?;
            }
            let next = {
                let request = requested.recv();
                let frame = self.frames.recv();
                pin_mut!(request, frame);
                match select(request, frame).await {
                    Either::Left((request, _)) => Either::Left(request),
                    Either::Right((frame, _)) => Either::Right(frame),
                }
            };
            match next {
                Either::Left(Some(request)) => self.handle(request).await?,
                Either::Left(None) => return Ok(()),
                Either::Right(Some(frame)) => {
                    if let Incoming::Event = self.incoming(&frame) {
                        self.changed = true;
                    }
                }
                Either::Right(None) => return Err("the server hung up".to_owned()),
            }
        }
    }

    async fn handle(&mut self, request: AudioRequest) -> Result<(), String> {
        match request {
            AudioRequest::Step(_) | AudioRequest::Set(_) => {
                let Some(sink) = self.sink().await? else {
                    return Ok(());
                };
                let channels: Vec<u32> =
                    sink.cvolume.channels().iter().map(Volume::as_u32).collect();
                let (target, louder) = match request {
                    AudioRequest::Set(level) => (set(&channels, level), level > 0.0),
                    AudioRequest::Step(by) => (stepped(&channels, by), by > 0.0),
                    AudioRequest::ToggleMute | AudioRequest::ToggleMicMute => return Ok(()),
                };
                if target != channels {
                    let mut volume = ChannelVolume::empty();
                    for channel in target {
                        volume.push(Volume::from_u32_clamped(channel));
                    }
                    self.acknowledged(Command::SetSinkVolume(SetDeviceVolumeParams {
                        device_index: Some(sink.index),
                        device_name: None,
                        volume,
                    }))
                    .await?
                    .map_err(|error| format!("the volume was not set: {error:?}"))?;
                }
                if louder && sink.muted {
                    self.mute_sink(sink.index, false).await?;
                }
            }
            AudioRequest::ToggleMute => {
                if let Some(sink) = self.sink().await? {
                    self.mute_sink(sink.index, !sink.muted).await?;
                }
            }
            AudioRequest::ToggleMicMute => {
                if let Some(source) = self.source().await? {
                    self.acknowledged(Command::SetSourceMute(SetDeviceMuteParams {
                        device_index: Some(source.index),
                        device_name: None,
                        mute: !source.muted,
                    }))
                    .await?
                    .map_err(|error| format!("the microphone was not muted: {error:?}"))?;
                }
            }
        }
        self.changed = true;
        Ok(())
    }

    async fn mute_sink(&mut self, index: u32, mute: bool) -> Result<(), String> {
        self.acknowledged(Command::SetSinkMute(SetDeviceMuteParams {
            device_index: Some(index),
            device_name: None,
            mute,
        }))
        .await?
        .map_err(|error| format!("the sound was not muted: {error:?}"))
    }

    async fn publish(&mut self, events: &WakingSender<MediaEvent>) -> Result<(), String> {
        let output = self.sink().await?.map(|sink| {
            let channels: Vec<u32> = sink.cvolume.channels().iter().map(Volume::as_u32).collect();
            level(&channels, sink.muted)
        });
        let input = self.source().await?.map(|source| {
            let channels: Vec<u32> = source
                .cvolume
                .channels()
                .iter()
                .map(Volume::as_u32)
                .collect();
            level(&channels, source.muted)
        });
        let _ = events.send(MediaEvent::Audio { output, input });
        Ok(())
    }

    async fn sink(&mut self) -> Result<Option<SinkInfo>, String> {
        let asked = Command::GetSinkInfo(GetSinkInfo {
            index: None,
            name: Some(CString::from(protocol::DEFAULT_SINK)),
        });
        Ok(self.reply(asked).await?.ok())
    }

    async fn source(&mut self) -> Result<Option<SourceInfo>, String> {
        let asked = Command::GetSourceInfo(GetSourceInfo {
            index: None,
            name: Some(CString::from(protocol::DEFAULT_SOURCE)),
        });
        Ok(self.reply(asked).await?.ok())
    }

    async fn reply<T: CommandReply>(
        &mut self,
        command: Command,
    ) -> Result<Result<T, PulseError>, String> {
        let frame = match self.send(command).await? {
            Ok(frame) => frame,
            Err(error) => return Ok(Err(error)),
        };
        let (_, reply) =
            protocol::read_reply_message::<T>(&mut Cursor::new(&frame[..]), self.version)
                .map_err(|error| format!("a reply could not be read: {error}"))?;
        Ok(Ok(reply))
    }

    async fn acknowledged(&mut self, command: Command) -> Result<Result<(), PulseError>, String> {
        Ok(self.send(command).await?.map(|_| ()))
    }

    async fn send(&mut self, command: Command) -> Result<Result<Vec<u8>, PulseError>, String> {
        let sequence = self.sequence;
        self.sequence = self.sequence.wrapping_add(1);
        let mut message = Vec::new();
        protocol::write_command_message(&mut message, sequence, &command, self.version)
            .map_err(|error| format!("a command could not be written: {error}"))?;
        (&*self.stream)
            .write_all(&message)
            .await
            .map_err(|error| format!("a command could not be sent: {error}"))?;
        loop {
            let frame = self
                .frames
                .recv()
                .await
                .ok_or_else(|| "the server hung up".to_owned())?;
            match self.incoming(&frame) {
                Incoming::Reply(answered) if answered == sequence => return Ok(Ok(frame)),
                Incoming::Error(answered, error) if answered == sequence => return Ok(Err(error)),
                Incoming::Event => self.changed = true,
                Incoming::Reply(_) | Incoming::Error(..) | Incoming::Other => {}
            }
        }
    }

    fn incoming(&self, frame: &[u8]) -> Incoming {
        match protocol::read_command_message(&mut Cursor::new(frame), self.version) {
            Ok((sequence, Command::Reply)) => Incoming::Reply(sequence),
            Ok((sequence, Command::Error(error))) => Incoming::Error(sequence, error),
            Ok((_, Command::SubscribeEvent(_))) => Incoming::Event,
            Ok(_) | Err(_) => Incoming::Other,
        }
    }
}

async fn read_frames(stream: Arc<Async<UnixStream>>, frames: UnboundedSender<Vec<u8>>) {
    loop {
        let mut frame = vec![0; protocol::DESCRIPTOR_SIZE];
        if (&*stream).read_exact(&mut frame).await.is_err() {
            return;
        }
        let length = u32::from_be_bytes([frame[0], frame[1], frame[2], frame[3]]) as usize;
        let channel = u32::from_be_bytes([frame[4], frame[5], frame[6], frame[7]]);
        if length > LONGEST_FRAME {
            eprintln!("block-app: PulseAudio sent a frame of {length} bytes");
            return;
        }
        frame.resize(protocol::DESCRIPTOR_SIZE + length, 0);
        if (&*stream)
            .read_exact(&mut frame[protocol::DESCRIPTOR_SIZE..])
            .await
            .is_err()
        {
            return;
        }
        if channel == u32::MAX && frames.send(frame).is_err() {
            return;
        }
    }
}
