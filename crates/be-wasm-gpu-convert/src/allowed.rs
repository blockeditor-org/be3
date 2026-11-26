//! What an app device's requested features allow, beyond what wgpu checks against the
//! device it runs on: the guest (be-wasm-gpu-guest) and the host (be-wasm-gpu-host's
//! wasi:webgpu) both check an app's calls against the features it asked for, not the host's.

#[derive(Debug, Clone, Copy)]
pub struct Allowed {
    pub requested: wgpu::Features,
    pub offered: wgpu::Features,
}

impl Allowed {
    pub fn query_set(&self, kind: wgpu::QueryType) -> Result<(), String> {
        match kind {
            wgpu::QueryType::Timestamp => self.needs(wgpu::Features::TIMESTAMP_QUERY),
            _ => Ok(()),
        }
    }

    fn needs(&self, needed: wgpu::Features) -> Result<(), String> {
        let missing = needed & !self.requested;
        match missing.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "Features {missing:?} are required but not enabled on the device"
            )),
        }
    }

    fn format_features(&self, format: wgpu::TextureFormat) -> [wgpu::TextureFormatFeatures; 2] {
        [
            format.guaranteed_format_features(self.requested),
            format.guaranteed_format_features(self.offered),
        ]
    }

    pub fn texture(&self, descriptor: &wgpu::TextureDescriptor<'_>) -> Result<(), String> {
        for format in std::iter::once(&descriptor.format).chain(descriptor.view_formats) {
            self.needs(format.required_features())?;
        }
        let [requested, offered] = self.format_features(descriptor.format);
        let refused = descriptor
            .usage
            .difference(requested.allowed_usages)
            .intersection(offered.allowed_usages);
        match refused.is_empty() {
            true => Ok(()),
            false => Err(format!(
                "Texture usages {refused:?} of format {:?} need a feature that is not enabled on the device",
                descriptor.format
            )),
        }
    }

    pub fn group_layout(
        &self,
        descriptor: &wgpu::BindGroupLayoutDescriptor<'_>,
    ) -> Result<(), String> {
        for entry in descriptor.entries {
            if let wgpu::BindingType::StorageTexture { format, .. } = entry.ty {
                self.needs(format.required_features())?;
                let [requested, offered] = self.format_features(format);
                let storage = wgpu::TextureUsages::STORAGE_BINDING;
                if !requested.allowed_usages.contains(storage)
                    && offered.allowed_usages.contains(storage)
                {
                    return Err(format!(
                        "Storage textures of format {format:?} need a feature that is not enabled on the device"
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn render_pipeline(
        &self,
        descriptor: &wgpu::RenderPipelineDescriptor<'_>,
    ) -> Result<(), String> {
        if descriptor.primitive.unclipped_depth {
            self.needs(wgpu::Features::DEPTH_CLIP_CONTROL)?;
        }
        let Some(fragment) = &descriptor.fragment else {
            return Ok(());
        };
        fragment
            .targets
            .iter()
            .flatten()
            .try_for_each(|target| self.color_target(target))
    }

    pub fn color_target(&self, target: &wgpu::ColorTargetState) -> Result<(), String> {
        let Some(blend) = target.blend else {
            return Ok(());
        };
        let factors = [
            blend.color.src_factor,
            blend.color.dst_factor,
            blend.alpha.src_factor,
            blend.alpha.dst_factor,
        ];
        if factors
            .iter()
            .any(wgpu::BlendFactor::ref_second_blend_source)
        {
            self.needs(wgpu::Features::DUAL_SOURCE_BLENDING)?;
        }
        let [requested, offered] = self.format_features(target.format);
        let blendable = wgpu::TextureFormatFeatureFlags::BLENDABLE;
        match !requested.flags.contains(blendable) && offered.flags.contains(blendable) {
            true => Err(format!(
                "Blending on format {:?} needs a feature that is not enabled on the device",
                target.format
            )),
            false => Ok(()),
        }
    }

    pub fn shader(&self, wgsl: &str) -> Result<(), String> {
        for extension in enabled(wgsl) {
            let needed = match extension {
                "f16" => wgpu::Features::SHADER_F16,
                "clip_distances" => wgpu::Features::CLIP_DISTANCES,
                "dual_source_blending" => wgpu::Features::DUAL_SOURCE_BLENDING,
                "primitive_index" => wgpu::Features::PRIMITIVE_INDEX,
                _ => continue,
            };
            self.needs(needed)?;
        }
        Ok(())
    }
}

pub fn enabled(wgsl: &str) -> Vec<&str> {
    let mut extensions = Vec::new();
    let mut rest = wgsl;
    loop {
        rest = skip_blank(rest);
        let word_end = rest
            .find(|character: char| !(character.is_alphanumeric() || character == '_'))
            .unwrap_or(rest.len());
        let (word, after) = rest.split_at(word_end);
        if !matches!(word, "enable" | "requires" | "diagnostic") {
            return extensions;
        }
        let Some(end) = after.find(';') else {
            return extensions;
        };
        if word == "enable" {
            extensions.extend(
                after[..end]
                    .split(',')
                    .map(|name| skip_blank(name).trim_end())
                    .filter(|name| !name.is_empty()),
            );
        }
        rest = &after[end + 1..];
    }
}

fn skip_blank(mut text: &str) -> &str {
    loop {
        text = text.trim_start();
        if let Some(line) = text.strip_prefix("//") {
            text = line.find('\n').map_or("", |end| &line[end..]);
        } else if text.starts_with("/*") {
            let mut depth = 0usize;
            let mut index = 0;
            let bytes = text.as_bytes();
            while index < bytes.len() {
                if bytes[index..].starts_with(b"/*") {
                    depth += 1;
                    index += 2;
                } else if bytes[index..].starts_with(b"*/") {
                    depth -= 1;
                    index += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    index += 1;
                }
            }
            text = &text[index..];
        } else {
            return text;
        }
    }
}
