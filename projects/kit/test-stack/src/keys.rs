//! Every key the stack stores, in one place. A key starts with its scope, so one project's pieces
//! sit side by side in TiKV and a scan of `p:<project>/` finds them all. There is no model file:
//! a model is these pieces.

/// Log numbers are padded so TiKV's byte order is also the log's order.
pub fn log(project: &str, seq: u64) -> String {
    format!("p:{project}/log/{seq:020}")
}
/// The last log number given out. Every commit changes this key, so TiKV lets only one writer win
/// each number, however many servers are running.
pub fn head(project: &str) -> String {
    format!("p:{project}/head")
}
/// An element's data, named by the hash of its bytes, so a version is never rewritten.
pub fn record(project: &str, hash: &str) -> String {
    format!("p:{project}/record/{hash}")
}
/// A shape, named by hash too: every door of one type shares it.
pub fn shape(project: &str, hash: &str) -> String {
    format!("p:{project}/shape/{hash}")
}
/// Which record is an element's current version.
pub fn elem(project: &str, model: &str, element: &str) -> String {
    format!("p:{project}/elem/{model}/{element}")
}
pub fn elem_prefix(project: &str, model: &str) -> String {
    format!("p:{project}/elem/{model}/")
}
/// Which elements touch a cell, so a browser loads only what is in view.
pub fn area(project: &str, cell: &str, element: &str) -> String {
    format!("p:{project}/area/{cell}/{element}")
}
pub fn area_prefix(project: &str, cell: &str) -> String {
    format!("p:{project}/area/{cell}/")
}
pub fn claim(project: &str, model: &str, element: &str) -> String {
    format!("p:{project}/claim/{model}/{element}")
}
pub fn claim_prefix(project: &str, model: &str) -> String {
    format!("p:{project}/claim/{model}/")
}
/// The last edit taken from one browser tab, so a resent edit is never applied twice.
pub fn client(project: &str, person: &str, client: &str) -> String {
    format!("p:{project}/client/{person}/{client}")
}
pub fn model(project: &str, model: &str) -> String {
    format!("p:{project}/model/{model}")
}
pub fn model_prefix(project: &str) -> String {
    format!("p:{project}/model/")
}
pub fn team(project: &str, team: &str) -> String {
    format!("p:{project}/team/{team}")
}
pub fn team_prefix(project: &str) -> String {
    format!("p:{project}/team/")
}
/// A package in a team's Shared or Consumed folder.
pub fn item(project: &str, team: &str, folder: &str, item: &str) -> String {
    format!("p:{project}/item/{team}/{folder}/{item}")
}
pub fn item_prefix(project: &str, team: &str, folder: &str) -> String {
    format!("p:{project}/item/{team}/{folder}/")
}
/// The next package number for a model.
pub fn version(project: &str, model: &str) -> String {
    format!("p:{project}/version/{model}")
}
/// A package bridged in and waiting for the team to take it.
pub fn incoming(project: &str, id: &str) -> String {
    format!("p:{project}/incoming/{id}")
}
pub fn incoming_prefix(project: &str) -> String {
    format!("p:{project}/incoming/")
}
pub fn link(project: &str, model: &str, id: &str) -> String {
    format!("p:{project}/link/{model}/{id}")
}
pub fn link_prefix(project: &str, model: &str) -> String {
    format!("p:{project}/link/{model}/")
}

pub fn person(id: &str) -> String {
    format!("dir/person/{id}")
}
pub fn email(address: &str) -> String {
    format!("dir/email/{address}")
}
/// One login, bound to its person the first time it signs in.
pub fn login(issuer_and_subject_hash: &str) -> String {
    format!("dir/login/{issuer_and_subject_hash}")
}
pub fn hub(id: &str) -> String {
    format!("dir/hub/{id}")
}
pub fn hub_project(hub: &str, project: &str) -> String {
    format!("dir/hub/{hub}/project/{project}")
}
pub fn hub_project_prefix(hub: &str) -> String {
    format!("dir/hub/{hub}/project/")
}
pub fn project(id: &str) -> String {
    format!("dir/project/{id}")
}

/// What a person belongs to. The value is the role; the key alone is what Cedar reads.
pub fn acl_person_prefix(person: &str) -> String {
    format!("acl/person/{person}/")
}
pub fn acl_hub(person: &str, hub: &str) -> String {
    format!("acl/person/{person}/hub/{hub}")
}
pub fn acl_project(person: &str, project: &str) -> String {
    format!("acl/person/{person}/project/{project}")
}
pub fn acl_team(person: &str, project: &str, team: &str) -> String {
    format!("acl/person/{person}/team/{project}/{team}")
}
/// The same memberships looked up from the hub's side.
pub fn acl_hub_user(hub: &str, person: &str) -> String {
    format!("acl/hub/{hub}/user/{person}")
}
pub fn acl_hub_user_prefix(hub: &str) -> String {
    format!("acl/hub/{hub}/user/")
}
/// `hub` permits bridges with `other`. A bridge needs this row from both sides.
pub fn acl_bridge(hub: &str, other: &str) -> String {
    format!("acl/hub/{hub}/bridge/{other}")
}
pub fn acl_bridge_prefix(hub: &str) -> String {
    format!("acl/hub/{hub}/bridge/")
}

/// A signed-in browser, under the hash of its cookie.
pub fn session(token_hash: &str) -> String {
    format!("auth/session/{token_hash}")
}
/// A sign-in on its way to Google and back.
pub fn pending(state: &str) -> String {
    format!("auth/pending/{state}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_keys_sort_in_log_order() {
        assert!(log("x", 9) < log("x", 10));
        assert!(log("x", 99_999) < log("x", 100_000));
    }

    #[test]
    fn every_project_key_starts_with_its_scope() {
        for key in [
            head("x"),
            log("x", 1),
            record("x", "ab"),
            area("x", "c", "e"),
        ] {
            assert!(key.starts_with("p:x/"), "{key}");
        }
    }
}
