use std::borrow::Cow;
use std::io;

use crate::job::Job;
use crate::Label;
use ptree;

#[derive(Debug, Clone)]
pub struct Stage {
    pub name: Label,
    pub jobs: Vec<Job>,
}

impl Stage {
    fn find_status(&self) -> &str {
        // Priorities are:
        // 1. running
        // 2. failed
        // 3. success

        let job_statuses = self
            .jobs
            .iter()
            .map(|j| j.status.clone())
            .collect::<Vec<String>>();

        if job_statuses.iter().any(|s| s == "running") {
            return "running";
        }
        if job_statuses.iter().any(|s| s == "failed") {
            return "failed";
        }
        if job_statuses.iter().any(|s| s == "success") {
            return "success";
        }

        "unknown"
    }
}

impl ptree::TreeItem for Stage {
    type Child = Job;

    fn write_self<W: io::Write>(&self, f: &mut W, _style: &ptree::Style) -> io::Result<()> {
        write!(f, "{}", &self.name.to_string(self.find_status()))
    }

    fn children(&self) -> Cow<[Self::Child]> {
        Cow::from(&self.jobs)
    }
}
