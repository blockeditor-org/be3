use std::time::{SystemTime, UNIX_EPOCH};

use block_client::blocks::version_control_data::{CommitId, VersionControlData};
use block_editor_plugin::beui::NodeId;
use block_editor_plugin::beui::reactive::view;
use block_editor_plugin::{Creation, Editor};
use uuid::Uuid;

mod ui;

use ui::RepositoryView;

const SECONDS_PER_DAY: i64 = 86_400;

pub struct VersionControlDataApp;

impl block_editor_plugin::BeuiApp for VersionControlDataApp {
    fn view(editor: Editor) -> NodeId {
        view! {
            <RepositoryView editor={editor} />
        }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        let client = creation.client();
        let author = client.account_id();
        let data = VersionControlData::new(author, unix_seconds_now());
        Ok(client.create_block(data).id())
    }
}

fn unix_seconds_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs() as i64)
        .unwrap_or(0)
}

pub(crate) fn short_author(author: Uuid) -> String {
    author
        .simple()
        .to_string()
        .chars()
        .take(CommitId::SHORT_LEN)
        .collect()
}

pub(crate) fn format_commit_time(seconds: i64) -> String {
    let days = seconds.div_euclid(SECONDS_PER_DAY);
    let time_of_day = seconds.rem_euclid(SECONDS_PER_DAY);
    let (year, month, day) = civil_from_days(days);
    let hour = time_of_day / 3600;
    let minute = (time_of_day % 3600) / 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}")
}

fn civil_from_days(days_since_epoch: i64) -> (i32, u8, u8) {
    let days = days_since_epoch + 719_468;
    let era = if days >= 0 { days } else { days - 146_096 } / 146_097;
    let day_of_era = days - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era as i32 + era as i32 * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i32::from(month <= 2);
    (year, month as u8, day as u8)
}
