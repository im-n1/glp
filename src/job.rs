use crate::Label;
use humantime::format_duration;
use ptree;
/// Represents Gitlab pipeline job.
use std::borrow::Cow;
use std::io;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Job {
    pub id: String,
    pub name: Label,
    pub web_url: String,
    pub status: String,
    pub stage: String,
    pub started_at: Option<String>,
    pub duration: Option<Duration>,
}

impl ptree::TreeItem for Job {
    type Child = Self;

    fn write_self<W: io::Write>(&self, f: &mut W, _style: &ptree::Style) -> io::Result<()> {
        let duration_str = match self.duration {
            // Keep duration seconds and forget the subtle resolution.
            // Use "-" as fallback in case of no duration at all.
            Some(duration) => format_duration(Duration::from_secs(duration.as_secs())).to_string(),
            _ => "-".to_string(),
        };

        write!(
            f,
            "{} ({})",
            &self.name.to_string(&self.status),
            duration_str
        )
    }

    fn children(&self) -> Cow<[Self::Child]> {
        Cow::from(vec![])
    }
}
