use autograder_rust::github::Github;
use autograder_rust::config::GithubCfg;
use httpmock::prelude::*;
use zip::write::FileOptions;
use std::io::Write;

#[test]
fn github_action_results_mock() {
    let server = MockServer::start();
    let base = server.base_url(); // e.g. http://127.0.0.1:XXXXX

    // Prepare artifact zip bytes containing grade-results.json
    let mut buf = Vec::new();
    {
        let mut zipw = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let options = FileOptions::default();
        zipw.start_file("grade-results.json", options).unwrap();
        zipw.write_all(b"{\"grade\": 8.0}").unwrap();
        zipw.finish().unwrap();
    }

    // Mock artifacts list
    let artifacts_endpoint = server.mock(|when, then| {
        when.method(GET).path("/repos/orgx/projx-alice/actions/artifacts");
        then.status(200)
            .header("content-type", "application/json")
            .body(format!("{{\"artifacts\":[{{\"id\":1,\"archive_download_url\":\"{}/artifact.zip\",\"workflow_run\":{{\"id\":123}}}}]}}", base));
    });

    // Mock runs/jobs
    let jobs_endpoint = server.mock(|when, then| {
        when.method(GET).path("/repos/orgx/projx-alice/actions/runs/123/jobs");
        then.status(200)
            .header("content-type", "application/json")
            .body("{\"jobs\":[{\"id\":987}]}");
    });

    // Mock artifact.zip
    let artifact_zip_endpoint = server.mock(|when, then| {
        when.method(GET).path("/artifact.zip");
        then.status(200)
            .header("content-type", "application/zip")
            .body(buf.clone());
    });

    let cfg = GithubCfg { host_name: base, access_token: String::from("tok") };
    let gh = Github::new(cfg, "orgx".into(), "projx".into(), false).unwrap();
    let rr = gh.get_action_results("alice");
    assert_eq!(rr.score, 8);
    assert!(rr.comment.contains("/actions/runs/123#summary-987"));

    artifacts_endpoint.assert();
    jobs_endpoint.assert();
    artifact_zip_endpoint.assert();
}
