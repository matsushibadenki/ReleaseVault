use crate::vault::R;
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

pub const FILE: &str = ".github/workflows/build-updater.yml";
pub trait Transport {
    fn request(&mut self, method: &str, path: &str, body: Option<Value>) -> R<(u16, Value)>;
}
pub struct GuardedClient<'a> {
    pub client: Box<dyn Transport>,
    pub validate: &'a dyn Fn() -> R<()>,
}
impl Transport for GuardedClient<'_> {
    fn request(&mut self, method: &str, path: &str, body: Option<Value>) -> R<(u16, Value)> {
        (self.validate)()?;
        self.client.request(method, path, body)
    }
}
#[derive(Clone, Serialize, Deserialize)]
pub struct Preview {
    pub id: String,
    pub repo: String,
    pub repository: u64,
    pub installation: Option<u64>,
    pub base: String,
    pub base_sha: String,
    pub old_sha: Option<String>,
    pub before: String,
    pub after: String,
    pub diff: String,
    pub branch: String,
    pub pr: Option<u64>,
}
impl Preview {
    pub fn view(&self) -> Value {
        json!({"id":self.id,"repo":self.repo,"base":self.base,"baseSha":self.base_sha,"path":FILE,
            "diff":self.diff,"branch":self.branch,"pr":self.pr,"changed":self.before!=self.after})
    }
}
fn success(reply: (u16, Value)) -> R<Value> {
    match reply {
        (200..=299, data) => Ok(data),
        (401 | 403 | 429, _) => Err("workflow-denied".into()),
        _ => Err("workflow-network".into()),
    }
}
fn segment(raw: &str) -> String {
    let mut url = tauri::Url::parse("https://github.com/").expect("fixed URL");
    url.path_segments_mut().expect("fixed URL").push(raw);
    url.path().trim_start_matches('/').to_owned()
}
fn query(pairs: &[(&str, &str)]) -> String {
    let mut url = tauri::Url::parse("https://github.com/").expect("fixed URL");
    url.query_pairs_mut().extend_pairs(pairs.iter().copied());
    url.query().unwrap_or("").into()
}
fn sha(value: &Value) -> R<String> {
    let sha = value.as_str().ok_or("workflow-network")?;
    if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("workflow-network".into());
    }
    Ok(sha.into())
}
fn head(client: &mut dyn Transport, repo: &str, branch: &str) -> R<Option<String>> {
    let result = client.request(
        "GET",
        &format!("/repos/{repo}/git/ref/heads/{}", segment(branch)),
        None,
    )?;
    if result.0 == 404 {
        return Ok(None);
    }
    sha(&success(result)?["object"]["sha"]).map(Some)
}
fn contents(client: &mut dyn Transport, repo: &str, commit: &str) -> R<(Option<String>, String)> {
    let result = client.request(
        "GET",
        &format!(
            "/repos/{repo}/contents/{FILE}?{}",
            query(&[("ref", commit)])
        ),
        None,
    )?;
    if result.0 == 404 {
        return Ok((None, String::new()));
    }
    let result = success(result)?;
    if result["type"] != "file"
        || result["encoding"] != "base64"
        || result["size"].as_u64().unwrap_or(u64::MAX) > 256 * 1024
    {
        return Err("workflow-file".into());
    }
    let encoded = result["content"]
        .as_str()
        .ok_or("workflow-file")?
        .replace(['\r', '\n'], "");
    let bytes = STANDARD.decode(encoded).map_err(|_| "workflow-file")?;
    if bytes.len() > 256 * 1024 {
        return Err("workflow-file".into());
    }
    Ok((
        Some(sha(&result["sha"])?),
        String::from_utf8(bytes).map_err(|_| "workflow-file")?,
    ))
}
pub fn preview(
    client: &mut dyn Transport,
    repo: &str,
    repository: u64,
    installation: Option<u64>,
    after: String,
) -> R<Preview> {
    crate::integrations::dest(repo, "")?;
    let info = success(client.request("GET", &format!("/repos/{repo}"), None)?)?;
    if !info["full_name"]
        .as_str()
        .is_some_and(|name| name.eq_ignore_ascii_case(repo))
        || (installation.is_some() && info["id"].as_u64() != Some(repository))
    {
        return Err("workflow-stale".into());
    }
    let base = info["default_branch"]
        .as_str()
        .filter(|b| !b.is_empty() && b.len() < 256 && !b.contains(".."))
        .ok_or("workflow-file")?
        .to_owned();
    let base_sha = head(client, repo, &base)?.ok_or("workflow-file")?;
    let (old_sha, before) = contents(client, repo, &base_sha)?;
    let mut digest = openssl::sha::Sha256::new();
    digest.update(repo.as_bytes());
    digest.update(base_sha.as_bytes());
    digest.update(after.as_bytes());
    let digest = digest
        .finish()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    Ok(Preview {
        id: STANDARD.encode(crate::vault::random::<24>()),
        repo: repo.into(),
        repository: info["id"]
            .as_u64()
            .filter(|id| *id > 0)
            .ok_or("workflow-network")?,
        installation,
        diff: diff(&before, &after, old_sha.is_some()),
        before,
        after,
        base,
        base_sha,
        old_sha,
        branch: format!("releasevault/updater-{}", &digest[..20]),
        pr: None,
    })
}
pub fn create(client: &mut dyn Transport, p: &Preview) -> R<u64> {
    crate::integrations::dest(&p.repo, "")?;
    if p.before == p.after {
        return Err("workflow-identical".into());
    }
    let info = success(client.request("GET", &format!("/repos/{}", p.repo), None)?)?;
    if info["id"].as_u64() != Some(p.repository)
        || info["default_branch"] != p.base
        || !info["full_name"]
            .as_str()
            .is_some_and(|name| name.eq_ignore_ascii_case(&p.repo))
    {
        return Err("workflow-stale".into());
    }
    if head(client, &p.repo, &p.base)?.as_deref() != Some(&p.base_sha) {
        return Err("workflow-stale".into());
    }
    let mut branch_sha = head(client, &p.repo, &p.branch)?;
    if branch_sha.is_none() {
        let response = client.request(
            "POST",
            &format!("/repos/{}/git/refs", p.repo),
            Some(json!({"ref":format!("refs/heads/{}",p.branch),"sha":p.base_sha})),
        )?;
        if response.0 != 422 {
            success(response)?;
        }
        branch_sha = head(client, &p.repo, &p.branch)?;
    }
    let branch_sha = branch_sha.ok_or("workflow-network")?;
    if branch_sha == p.base_sha {
        let mut body = json!({"message":"Update signed updater build workflow","content":STANDARD.encode(p.after.as_bytes()),"branch":p.branch});
        if let Some(sha) = &p.old_sha {
            body["sha"] = json!(sha);
        }
        success(client.request(
            "PUT",
            &format!("/repos/{}/contents/{FILE}", p.repo),
            Some(body),
        )?)?;
    }
    let branch_sha = if branch_sha == p.base_sha {
        head(client, &p.repo, &p.branch)?.ok_or("workflow-network")?
    } else {
        branch_sha
    };
    // Check both newly written and recovered commits before proposing their contents.
    let commit = success(client.request(
        "GET",
        &format!("/repos/{}/commits/{branch_sha}", p.repo),
        None,
    )?)?;
    let parents = commit["parents"].as_array().ok_or("workflow-stale")?;
    let files = commit["files"].as_array().ok_or("workflow-stale")?;
    if parents.len() != 1
        || parents[0]["sha"] != p.base_sha
        || files.len() != 1
        || files[0]["filename"] != FILE
        || contents(client, &p.repo, &branch_sha)?.1 != p.after
    {
        return Err("workflow-stale".into());
    }
    if head(client, &p.repo, &p.base)?.as_deref() != Some(&p.base_sha) {
        return Err("workflow-stale".into());
    }
    let owner = p.repo.split('/').next().ok_or("destination")?;
    let head_filter = format!("{owner}:{}", p.branch);
    let path = format!(
        "/repos/{}/pulls?{}",
        p.repo,
        query(&[
            ("state", "all"),
            ("head", &head_filter),
            ("base", &p.base),
            ("per_page", "100")
        ])
    );
    let prs = success(client.request("GET", &path, None)?)?;
    for pr in prs.as_array().ok_or("workflow-network")? {
        if pr["head"]["ref"] == p.branch
            && pr["base"]["ref"] == p.base
            && pr["head"]["repo"]["full_name"]
                .as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case(&p.repo))
            && pr["base"]["repo"]["full_name"]
                .as_str()
                .is_some_and(|r| r.eq_ignore_ascii_case(&p.repo))
        {
            return pr["number"]
                .as_u64()
                .filter(|n| *n > 0)
                .ok_or("workflow-network".into());
        }
    }
    let body = json!({"title":"Build signed updater artifacts across platforms","head":p.branch,"base":p.base,"draft":true,
        "body":"Adds the reviewed ReleaseVault build workflow. It runs manually, builds the selected platforms, checks updater artifacts and signature files, and collects the output.\n\nConfigure the selected GitHub Environment secrets and platform signing settings before running this workflow."});
    let created =
        success(client.request("POST", &format!("/repos/{}/pulls", p.repo), Some(body))?)?;
    created["number"]
        .as_u64()
        .filter(|n| *n > 0)
        .ok_or("workflow-network".into())
}
pub fn diff(before: &str, after: &str, existed: bool) -> String {
    if before == after {
        return String::new();
    }
    let old: Vec<_> = before.split_inclusive('\n').collect();
    let new: Vec<_> = after.split_inclusive('\n').collect();
    let prefix = old.iter().zip(&new).take_while(|(a, b)| a == b).count();
    let suffix = old[prefix..]
        .iter()
        .rev()
        .zip(new[prefix..].iter().rev())
        .take_while(|(a, b)| a == b)
        .count();
    let start = prefix.saturating_sub(3);
    let old_end = old.len() - suffix + suffix.min(3);
    let new_end = new.len() - suffix + suffix.min(3);
    let old_count = old_end - start;
    let new_count = new_end - start;
    let mut result = format!(
        "--- {}\n+++ b/{FILE}\n@@ -{},{} +{},{} @@\n",
        if existed {
            format!("a/{FILE}")
        } else {
            "/dev/null".into()
        },
        if old_count == 0 { start } else { start + 1 },
        old_count,
        if new_count == 0 { start } else { start + 1 },
        new_count
    );
    let mut line = |prefix: char, text: &str| {
        result.push(prefix);
        result.push_str(text);
        if !text.ends_with('\n') {
            result.push_str("\n\\ No newline at end of file\n");
        }
    };
    for text in &old[start..prefix] {
        line(' ', text);
    }
    for text in &old[prefix..old.len() - suffix] {
        line('-', text);
    }
    for text in &new[prefix..new.len() - suffix] {
        line('+', text);
    }
    for text in &old[old.len() - suffix..old_end] {
        line(' ', text);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;
    const BASE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const COMMIT: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const BLOB: &str = "cccccccccccccccccccccccccccccccccccccccc";
    struct Fixture {
        replies: VecDeque<(&'static str, u16, Value)>,
        requests: Vec<(String, String, Option<Value>)>,
    }
    impl Transport for Fixture {
        fn request(&mut self, method: &str, path: &str, body: Option<Value>) -> R<(u16, Value)> {
            let (expected, status, data) = self.replies.pop_front().expect("unexpected request");
            assert!(
                format!("{method} {path}").starts_with(expected),
                "{method} {path}"
            );
            self.requests.push((method.into(), path.into(), body));
            Ok((status, data))
        }
    }
    fn fixture(replies: Vec<(&'static str, u16, Value)>) -> Fixture {
        Fixture {
            replies: replies.into(),
            requests: vec![],
        }
    }
    fn info() -> Value {
        json!({"id":17,"full_name":"studio/mobile","default_branch":"main"})
    }
    fn reference(sha: &str) -> Value {
        json!({"object":{"sha":sha}})
    }
    fn file(text: &str) -> Value {
        json!({"type":"file","encoding":"base64","size":text.len(),"sha":BLOB,"content":STANDARD.encode(text)})
    }
    fn prepared() -> Preview {
        let mut client = fixture(vec![
            ("GET /repos/studio/mobile", 200, info()),
            (
                "GET /repos/studio/mobile/git/ref/heads/main",
                200,
                reference(BASE),
            ),
            ("GET /repos/studio/mobile/contents/", 200, file("old\n")),
        ]);
        let preview = preview(&mut client, "studio/mobile", 17, Some(100), "new\n".into()).unwrap();
        assert!(client.requests.iter().all(|r| r.0 == "GET"));
        assert_eq!(preview.repository, 17);
        preview
    }
    #[test]
    fn preview_and_create_change_only_reviewed_file_on_dedicated_branch() {
        let p = prepared();
        let mut client = fixture(vec![
            ("GET /repos/studio/mobile", 200, info()),
            (
                "GET /repos/studio/mobile/git/ref/heads/main",
                200,
                reference(BASE),
            ),
            (
                "GET /repos/studio/mobile/git/ref/heads/releasevault",
                404,
                Value::Null,
            ),
            ("POST /repos/studio/mobile/git/refs", 201, json!({})),
            (
                "GET /repos/studio/mobile/git/ref/heads/releasevault",
                200,
                reference(BASE),
            ),
            ("PUT /repos/studio/mobile/contents/", 200, json!({})),
            (
                "GET /repos/studio/mobile/git/ref/heads/releasevault",
                200,
                reference(COMMIT),
            ),
            (
                "GET /repos/studio/mobile/commits/",
                200,
                json!({"parents":[{"sha":BASE}],"files":[{"filename":FILE}]}),
            ),
            ("GET /repos/studio/mobile/contents/", 200, file("new\n")),
            (
                "GET /repos/studio/mobile/git/ref/heads/main",
                200,
                reference(BASE),
            ),
            ("GET /repos/studio/mobile/pulls?", 200, json!([])),
            ("POST /repos/studio/mobile/pulls", 201, json!({"number":9})),
        ]);
        assert_eq!(create(&mut client, &p).unwrap(), 9);
        assert!(client.replies.is_empty());
        let put = client.requests.iter().find(|r| r.0 == "PUT").unwrap();
        assert_eq!(put.1, format!("/repos/studio/mobile/contents/{FILE}"));
        assert_eq!(put.2.as_ref().unwrap()["branch"], p.branch);
        assert_eq!(put.2.as_ref().unwrap()["sha"], BLOB);
        assert_eq!(put.2.as_ref().unwrap()["content"], STANDARD.encode("new\n"));
        let pr = client.requests.last().unwrap().2.as_ref().unwrap();
        assert_eq!(pr["draft"], true);
        assert_eq!(pr["base"], "main");
    }
    #[test]
    fn retry_recovers_existing_commit_and_pr_without_duplicate_writes() {
        let p = prepared();
        let mut client = fixture(vec![
            ("GET /repos/studio/mobile", 200, info()),
            (
                "GET /repos/studio/mobile/git/ref/heads/main",
                200,
                reference(BASE),
            ),
            (
                "GET /repos/studio/mobile/git/ref/heads/releasevault",
                200,
                reference(COMMIT),
            ),
            (
                "GET /repos/studio/mobile/commits/",
                200,
                json!({"parents":[{"sha":BASE}],"files":[{"filename":FILE}]}),
            ),
            ("GET /repos/studio/mobile/contents/", 200, file("new\n")),
            (
                "GET /repos/studio/mobile/git/ref/heads/main",
                200,
                reference(BASE),
            ),
            (
                "GET /repos/studio/mobile/pulls?",
                200,
                json!([{"number":9,"head":{"ref":p.branch,"repo":{"full_name":p.repo}},"base":{"ref":p.base,"repo":{"full_name":p.repo}}}]),
            ),
        ]);
        assert_eq!(create(&mut client, &p).unwrap(), 9);
        assert!(client.requests.iter().all(|r| r.0 == "GET"));
        let mut stale = fixture(vec![
            ("GET /repos/studio/mobile", 200, info()),
            (
                "GET /repos/studio/mobile/git/ref/heads/main",
                200,
                reference(COMMIT),
            ),
        ]);
        assert_eq!(create(&mut stale, &p).unwrap_err(), "workflow-stale");
        assert!(stale.requests.iter().all(|r| r.0 == "GET"));
    }
    #[test]
    fn guard_blocks_revoked_sessions_and_diff_handles_line_endings() {
        let mut guard = GuardedClient {
            client: Box::new(fixture(vec![])),
            validate: &|| Err("locked".into()),
        };
        assert_eq!(
            guard
                .request("POST", "/repos/studio/mobile/pulls", None)
                .unwrap_err(),
            "locked"
        );
        assert_eq!(diff("same\n", "same\n", true), "");
        assert!(diff("", "new\n", false).contains("--- /dev/null"));
        assert!(diff("before", "after", true).contains(r"\ No newline at end of file"));
        assert!(diff("a\nb\nc\n", "a\nchanged\nc\n", true).contains("-b\n+changed\n"));
        if let Ok(dir) = std::env::var("RV_PR_DIFF_QA") {
            std::fs::create_dir_all(&dir).unwrap();
            for (index, (before, after)) in [
                ("", "new\n"),
                ("before", "after"),
                ("a\nb\nc\n", "a\nchanged\nc\n"),
                ("delete\n", ""),
            ]
            .into_iter()
            .enumerate()
            {
                std::fs::write(format!("{dir}/{index}.before"), before).unwrap();
                std::fs::write(format!("{dir}/{index}.after"), after).unwrap();
                std::fs::write(format!("{dir}/{index}.patch"), diff(before, after, true)).unwrap();
            }
        }
    }
}
