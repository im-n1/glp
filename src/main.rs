mod args;
mod job;
mod pipeline;
mod stage;

use crate::job::Job;
use crate::pipeline::Pipeline;
use crate::stage::Stage;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::env;
use std::sync::Arc;
use std::time::Duration;

use colored::*;
use futures::future::join_all;
use json;
use ptree;
use reqwest;
use tokio::fs;
use tokio::sync::Semaphore;

const DEFAULT_LIMIT: u8 = 3;
const SEMAPHORE_LIMIT: usize = 10;

// trait Labelable {}

#[derive(Debug, Clone)]
pub struct Label(String);

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

// Represents Gitlab stage (group of jobs).

/// Takes following poritional arguments:
/// - project ID
#[tokio::main]
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 0. Parse arguments.
    let app_args = args::parse();
    let project_id = match app_args.get_one::<String>("project") {
        Some(id) => id.to_owned(),
        None => fs::read_to_string(".glp")
            .await
            .expect("No project ID (no parameter nor .glp file."),
    };
    let show_finished = app_args.get_one::<bool>("finished").unwrap().clone();

    let private_token = env::var("GLP_PRIVATE_TOKEN")
        .expect("No Gitlab private token found - set GLP_PRIVATE_TOKEN environment variable.");

    // 1. Fetch pipelines.
    let client = reqwest::Client::new();
    let response = client
        .get(format!(
            "https://gitlab.com/api/v4/projects/{}/pipelines?per_page={}",
            project_id,
            app_args.get_one::<u8>("limit").unwrap().to_string()
        ))
        .header("PRIVATE-TOKEN", &private_token)
        .send()
        .await?
        .text()
        .await?;

    let pipelines = json::parse(&response)?;

    // 2. Fetch jobs for each running pipeline.
    let mut tasks = vec![];
    let semaphore = Arc::new(Semaphore::new(SEMAPHORE_LIMIT));

    for i in 0..pipelines.len() {
        let pip = pipelines[i].clone();
        let private_token = private_token.clone();
        let project_id = project_id.clone();

        // Acquire semaphore lock.
        let semaphore_permit = semaphore.clone().acquire_owned().await.unwrap();

        tasks.push(tokio::spawn(async move {
            // Fetch jobs for current pipeline.
            let client = reqwest::Client::new();
            let response = client
                .get(format!(
                    "https://gitlab.com/api/v4/projects/{}/pipelines/{}/jobs",
                    project_id, pip["id"]
                ))
                .header("PRIVATE-TOKEN", &private_token)
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

            let mut pip = Pipeline {
                id: Label(pip["id"].as_usize().unwrap().to_string()),
                git_ref: pip["ref"].as_str().unwrap().to_string(),
                status: pip["status"].as_str().unwrap().to_string(),
                stages: pip_stages,
                show_finished,
                details: None,
            };

            // Fetch details only if needed.
            if show_finished {
                pip.fetch_details(&private_token, &project_id).await;
            }

            // Free acquired semaphore lock.
            drop(semaphore_permit);

            pip
        }));
    }

    let pips: Vec<_> = join_all(tasks)
        .await
        .into_iter()
        .map(|i| i.unwrap())
        .collect();

    // 3. Print tree.
    for (i, pip) in pips.iter().enumerate() {
        // Space between pipelines.
        if i > 0 {
            println!("")
        }

        ptree::output::print_tree(pip).unwrap();
    }

    Ok(())
}
