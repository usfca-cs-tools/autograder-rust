use std::fs;
use std::io::Write;

use autograder_rust::canvas::upload_class;
use autograder_rust::config::{CanvasCfg, CanvasMapperCfg};
use httpmock::prelude::*;

#[test]
fn canvas_upload_mock() {
    let server = MockServer::start();
    let base = server.base_url();
    let host = base; // includes scheme

    // Prepare class JSON
    let tmp = tempfile::tempdir().unwrap();
    let project = "projx";
    let json_path = tmp.path().join(format!("{}.json", project));
    fs::write(&json_path, r#"[
  {"student":"alice", "score": 8, "comment": "ok"},
  {"student":"bob",   "score": 10, "comment": "great"}
]"#).unwrap();

    // Prepare CSV mapping
    let csv_path = tmp.path().join("map.csv");
    let mut f = fs::File::create(&csv_path).unwrap();
    writeln!(f, "GitHub,SIS Login ID").unwrap();
    writeln!(f, "alice,a123").unwrap();
    writeln!(f, "bob,b456").unwrap();

    // Mock Canvas endpoints
    let courses = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses");
        then.status(200).header("content-type", "application/json")
            .body("[{\"id\":42,\"name\":\"Course X\"}]");
    });
    let assignments = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses/42/assignments");
        then.status(200).header("content-type", "application/json")
            .body("[{\"id\":7,\"name\":\"projx\"}]");
    });
    let enrollments = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses/42/enrollments");
        then.status(200).header("content-type", "application/json")
            .body("[{\"user_id\":101,\"user\":{\"login_id\":\"a123\"}},{\"user_id\":102,\"user\":{\"login_id\":\"b456\"}}]");
    });
    let sub_a = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses/42/assignments/7/submissions/101");
        then.status(200).header("content-type", "application/json")
            .body("{\"score\": 8.0}"); // skip alice
    });
    let sub_b = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses/42/assignments/7/submissions/102");
        then.status(200).header("content-type", "application/json")
            .body("{\"score\": 9.0}"); // update bob
    });
    let put_b = server.mock(|when, then| {
        when.method(PUT).path("/api/v1/courses/42/assignments/7/submissions/102");
        then.status(200);
    });

    let canvas = CanvasCfg { host_name: host, access_token: String::from("tok"), course_name: String::from("Course X") };
    let mapper = CanvasMapperCfg { map_path: csv_path.to_string_lossy().to_string(), github_col_name: String::from("GitHub"), login_col_name: String::from("SIS Login ID") };
    upload_class(canvas, mapper, project, Some(json_path.to_str().unwrap()), false).unwrap();

    courses.assert();
    assignments.assert();
    enrollments.assert();
    sub_a.assert();
    sub_b.assert();
    put_b.assert();
}

#[test]
fn canvas_pagination_and_error() {
    let server = MockServer::start();
    let base = server.base_url();
    let host = base;

    // Page 1 without target course, with Link header to page 2
    let courses_p1 = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses").query_param("per_page", "100");
        then.status(200)
            .header("content-type", "application/json")
            .header("Link", "</api/v1/courses?page=2>; rel=next")
            .body("[{\"id\":41,\"name\":\"Other\"}]");
    });
    let courses_p2 = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses").query_param("page", "2");
        then.status(200)
            .header("content-type", "application/json")
            .body("[{\"id\":42,\"name\":\"Course X\"}]");
    });

    // Next endpoints minimal for error test: assignments 500
    let assignments_500 = server.mock(|when, then| {
        when.method(GET).path("/api/v1/courses/42/assignments");
        then.status(500);
    });

    let canvas = CanvasCfg { host_name: host, access_token: String::from("tok"), course_name: String::from("Course X") };
    let mapper = CanvasMapperCfg { map_path: String::from("/nonexistent.csv"), github_col_name: String::from("GitHub"), login_col_name: String::from("SIS Login ID") };

    // Call get_course_id through upload_class minimal path: expect failure due to missing CSV
    // but ensure pagination was hit
    // We'll directly construct client and call get_course_id instead for precision
    let client = autograder_rust::canvas::CanvasClient::new(canvas, false).unwrap();
    let course_id = client.get_course_id().unwrap();
    assert_eq!(course_id, 42);
    courses_p1.assert(); courses_p2.assert();

    // Assignments 500 should error
    let err = client.get_assignment_id(course_id, "projx").unwrap_err();
    let msg = format!("{}", err);
    assert!(msg.contains("assignments GET failed"));
    assignments_500.assert();
}
