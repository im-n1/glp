use std::borrow::Cow;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::io;
use std::time::Duration;

use colored::*;
use futures::future::join_all;
use humantime::format_duration;
use json;
use ptree;
use reqwest;
use tokio::fs;

trait Labelable {}

#[derive(Debug, Clone)]
struct Label(String);

impl Label {
    fn to_string(&self, base: &str) -> String {
        match base {
            "success" => self.0.green().to_string(),
            "failed" => self.0.red().to_string().to_string(),
            "manual" => format!("{} [manual]", self.0),
            "running" => self.0.yellow().to_string(),
            &_ => format!("{}", self.0),
        }
    }
}

/// Represents Gitlab pipeline.
#[derive(Debug, Clone)]
struct Pipeline {
    id: Label,
    git_ref: String,
    status: String,
    stages: Vec<Stage>,
}

impl ptree::TreeItem for Pipeline {
    type Child = Stage;

    fn write_self<W: io::Write>(&self, f: &mut W, _style: &ptree::Style) -> io::Result<()> {
        write!(
            f,
            "{} ({})",
            &self.id.to_string(&self.status),
            &self.git_ref
        )
    }

    fn children(&self) -> Cow<[Self::Child]> {
        Cow::from(&self.stages)
    }
}

#[derive(Debug, Clone)]
struct Stage {
    name: Label,
    jobs: Vec<Job>,
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

/// Represents Gitlab pipeline job.
#[derive(Debug, Clone)]
struct Job {
    id: String,
    name: Label,
    web_url: String,
    status: String,
    stage: String,
    started_at: Option<String>,
    duration: Option<Duration>,
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

// fn print_type_of<T>(_: &T) {
//     println!("{}", std::any::type_name::<T>());
// }

/// Takes following poritional arguments:
/// - project ID
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = env::args().collect::<Vec<String>>();
    // Get project ID from first argument or .glp file.
    let project_id = match args.get(1) {
        Some(id) => id.to_owned(),
        None => fs::read_to_string(".glp")
            .await
            .expect("No project ID (no parameter nor .glp file."),
    };

    let private_token = env::var("GLP_PRIVATE_TOKEN").unwrap();
    let limit = 3;

    // 1. Fetch pipelines.
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "https://gitlab.com/api/v4/projects/{}/pipelines?per_page={}",
            project_id, limit
        ))
        .header("PRIVATE-TOKEN", &private_token)
        .send()
        .await?
        .text()
        .await?;

    // println!(
    //     "gitlab response: {}",
    //     response.chars().take(50).collect::<String>()
    // );
    let pipelines = json::parse(&response)?;
    // print_type_of(&pipelines);
    // println!("{:?}", &pipelines);

    // 2. Fetch jobs for each running pipeline.
    let mut tasks = vec![];

    for i in 0..pipelines.len() {
        let pip = pipelines[i].clone();
        let private_token = private_token.clone();
        let project_id = project_id.clone();
        // println!("processing pipeline {}", pip["id"]);

        tasks.push(tokio::spawn(async move {
            // Fetch jobs for current pipeline.
            let client = reqwest::Client::new();
            let response = client
                .get(format!(
                    "https://gitlab.com/api/v4/projects/{}/pipelines/{}/jobs",
                    project_id, pip["id"]
                ))
                .header("PRIVATE-TOKEN", private_token)
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();

            // println!(
            //     "gitlab response: {}",
            //     response.chars().take(50).collect::<String>()
            // );

            let jobs = json::parse(&response).unwrap();
            let mut stages: HashMap<String, Vec<Job>> = HashMap::new();

            for j in 0..jobs.len() {
                let job = &jobs[j];

                let pip_job = Job {
                    id: job["id"].as_usize().unwrap().to_string(),
                    name: Label(job["name"].as_str().unwrap().to_string()),
                    status: job["status"].as_str().unwrap().to_string(),
                    web_url: job["web_url"].as_str().unwrap().to_string(),
                    stage: job["stage"].as_str().unwrap().to_string(),
                    started_at: match job["started_at"].is_null() {
                        true => None,
                        false => Some(job["started_at"].as_str().unwrap().to_string()),
                    },
                    duration: match job["duration"].is_null() {
                        true => None,
                        false => Some(Duration::from_secs_f64(job["duration"].as_f64().unwrap())),
                    },
                };

                if stages.contains_key(&pip_job.stage) {
                    stages.get_mut(&pip_job.stage).unwrap().push(pip_job);
                } else {
                    stages.insert(pip_job.stage.clone(), vec![pip_job]);
                }
            }

            let mut pip_stages = vec![];

            // Convert hashmap to vec of stages
            for (stage, jobs) in stages.into_iter() {
                pip_stages.push(Stage {
                    name: Label(stage),
                    jobs,
                });
            }

            // Sort stages by job "started_at" times.
            // None are always classifiead as "greater"
            // so they end up as "last".
            pip_stages.sort_by(|a, b| {
                let mut a_jobs = a
                    .jobs
                    .iter()
                    .filter(|j| j.started_at.is_some())
                    .collect::<Vec<&Job>>();
                a_jobs.sort_by_key(|j| j.started_at.clone());

                let mut b_jobs = b
                    .jobs
                    .iter()
                    .filter(|j| j.started_at.is_some())
                    .collect::<Vec<&Job>>();
                b_jobs.sort_by_key(|j| j.started_at.clone());

                let a_started_at = match a_jobs.get(0) {
                    Some(j) => j.started_at.clone(),
                    _ => None,
                };
                let b_started_at = match b_jobs.get(0) {
                    Some(j) => j.started_at.clone(),
                    _ => None,
                };

                if a_started_at.is_none() {
                    return Ordering::Greater;
                }
                if b_started_at.is_none() {
                    return Ordering::Less;
                }

                return a_started_at.partial_cmp(&b_started_at).unwrap();
            });

            Pipeline {
                id: Label(pip["id"].as_usize().unwrap().to_string()),
                git_ref: pip["ref"].as_str().unwrap().to_string(),
                status: pip["status"].as_str().unwrap().to_string(),
                stages: pip_stages,
            }
        }));
    }

    let pips: Vec<_> = join_all(tasks)
        .await
        .into_iter()
        .map(|i| i.unwrap())
        .collect();

    // 3. Print tree.
    for pip in pips {
        ptree::output::print_tree(&pip).unwrap();
    }

    Ok(())
}
