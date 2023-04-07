use crate::stage::Stage;
use crate::Label;
use chrono::{offset::Local, DateTime};
use humantime::format_duration;
use json::JsonValue;
use ptree;
use reqwest;
use std::borrow::Cow;
use std::io;
use std::time::Duration;
use timeago;

/// Represents Gitlab pipeline.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub id: Label,
    pub git_ref: String,
    pub status: String,
    pub stages: Vec<Stage>,
    pub show_finished: bool,
    pub details: Option<JsonValue>,
}

impl ptree::TreeItem for Pipeline {
    type Child = Stage;

    fn write_self<W: io::Write>(&self, f: &mut W, _style: &ptree::Style) -> io::Result<()> {
        let mut suffix = String::new();

        if self.is_finished() {
            suffix = self.get_duration_suffix();
        }

        if self.show_finished {
            if let Some(finished) = self.get_finished_suffix() {
                suffix.push_str(finished.as_str());
            }
        }

        write!(
            f,
            "{} ({}){}",
            &self.id.to_string(&self.status),
            &self.git_ref,
            suffix
        )
    }

    fn children(&self) -> Cow<[Self::Child]> {
        Cow::from(&self.stages)
    }
}

impl Pipeline {
    fn is_finished(&self) -> bool {
        "success" == self.status || "failed" == self.status
    }

    /// Producess output like " [7m 2s]" as a sum of
    /// duration of all pipeline jobs.
    /// Truncate units lower than seconds.
    fn get_duration_suffix(&self) -> String {
        let mut sum = Duration::from_secs(0);

        // TODO: fetch from details
        for stage in self.stages.iter() {
            for job in stage.jobs.iter() {
                if let Some(dur) = job.duration {
                    sum += dur
                }
            }
        }

        format!(
            " [{}]",
            format_duration(Duration::from_secs(sum.as_secs())).to_string()
        )
    }

    /// Fetches pipeline details from Gitlab API.
    pub async fn fetch_details(&mut self, private_token: &str, project_id: &str) {
        // Fetch jobs for current pipeline.
        let client = reqwest::Client::new();
        let response = client
            .get(format!(
                "https://gitlab.com/api/v4/projects/{}/pipelines/{}",
                project_id, &self.id.0
            ))
            .header("PRIVATE-TOKEN", private_token)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap();

        self.details = Some(json::parse(&response).unwrap());
    }

    /// Calculates (if available) relative time when the
    /// pipeline has finished.
    /// Producess output like " [2 days ago]".
    fn get_finished_suffix(&self) -> Option<String> {
        let finished_at = self.details.as_ref().unwrap()["finished_at"].as_str();

        if finished_at.is_some() {
            let formatter = timeago::Formatter::new();

            return Some(format!(
                " [{}]",
                formatter.convert_chrono(
                    DateTime::parse_from_rfc3339(finished_at.unwrap())
                        .expect("Cannot parse pipeline \"finished_at\" field."),
                    Local::now()
                )
            ));
        }

        None
    }
}
