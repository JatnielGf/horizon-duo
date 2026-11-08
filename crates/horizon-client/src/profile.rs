//! Local player profiles: each one has its own game folder (options, saves,
//! servers, resource packs, mods and the offline player name), so two people
//! sharing one device keep separate settings.
//!
//! The game reads and writes everything relative to its working directory, so
//! entering a profile is just `set_current_dir(profile folder)` before the game
//! starts (see `main.rs`). Nothing else needs to know about profiles.
//!
//! A profile's name is also the player's name in the game: a server sees it, and a world keeps the
//! player's inventory under the UUID made from it (`offline_name` explains why every player needs
//! a name of their own). So it follows Minecraft's rules for a name, and it is the only place the
//! name of a profile is kept. The built-in profile has no name of its own: it keeps the offline
//! name `offline_name` generates once into its game folder, as before.
//!
//! The built-in `default` profile *is* the game folder itself. A device with no
//! `profiles.json`, or with `default` active, keeps every file where it was:
//! there is nothing to migrate and nothing to move. The other profiles live in
//! `profiles/profile-N/` next to the index. Logs and crash reports stay in the
//! game folder, whichever profile is active.
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
};

/// The index of the extra profiles, in the game folder.
pub const INDEX_FILE: &str = "profiles.json";
/// The folder of the extra profiles, in the game folder.
pub const DIR: &str = "profiles";
/// The built-in profile: the game folder itself.
pub const DEFAULT_ID: &str = "default";
pub const DEFAULT_NAME: &str = "Default";
/// The longest name, in characters: Minecraft's limit for a player's name.
pub const MAX_NAME_CHARS: usize = 16;
/// What an index written before names were players' names may hold.
const MAX_STORED_NAME_CHARS: usize = 32;
const ID_PREFIX: &str = "profile-";

/// The profile this run of the game is in (the built-in one until `main` enters
/// another).
static RUNNING: std::sync::Mutex<String> = std::sync::Mutex::new(String::new());

pub fn set_running(id: &str) {
    *RUNNING.lock().unwrap_or_else(|e| e.into_inner()) = id.to_owned();
}

pub fn running() -> String {
    let id = RUNNING.lock().unwrap_or_else(|e| e.into_inner()).clone();
    if id.is_empty() { DEFAULT_ID.into() } else { id }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Profile {
    pub id: String,
    pub name: String,
}

impl Profile {
    fn builtin() -> Self {
        Self {
            id: DEFAULT_ID.into(),
            name: DEFAULT_NAME.into(),
        }
    }

    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_ID
    }
}

/// What `profiles.json` holds: the extra profiles and the active one. The
/// built-in profile is never stored.
#[derive(Default, Serialize, Deserialize)]
struct Index {
    #[serde(default)]
    active: Option<String>,
    #[serde(default)]
    profiles: Vec<Profile>,
}

/// The profiles of one game folder.
pub struct Profiles {
    root: PathBuf,
    index: Index,
}

/// `profile-N` with N a positive number: the only ids that name a folder, so
/// an index edited by hand can't point outside `profiles/`.
fn is_extra_id(id: &str) -> bool {
    id.strip_prefix(ID_PREFIX).is_some_and(|n| {
        !n.is_empty()
            && n.len() <= 9
            && !n.starts_with('0')
            && n.bytes().all(|b| b.is_ascii_digit())
    })
}

/// The name as kept: trimmed, and a name a player can have in Minecraft (1 to 16 printable
/// characters, no spaces: `offline_name::is_valid`).
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.trim();
    crate::offline_name::is_valid(name).then(|| name.to_owned())
}

/// A name an index may hold: profiles made when any short label was allowed keep working.
fn is_stored_name(name: &str) -> bool {
    let chars = name.chars().count();
    name == name.trim()
        && (1..=MAX_STORED_NAME_CHARS).contains(&chars)
        && !name.chars().any(char::is_control)
}

/// The name `profile` plays under: its own when it is a Minecraft name, else the nearest one
/// (spaces and other characters a name can't have become `_`, and the name is cut to 16
/// characters). A name with nothing left of it is `Player` and the profile's number.
pub fn player_name(profile: &Profile) -> String {
    if crate::offline_name::is_valid(&profile.name) {
        return profile.name.clone();
    }
    let name: String = profile
        .name
        .chars()
        .map(|c| if c.is_ascii_graphic() { c } else { '_' })
        .take(MAX_NAME_CHARS)
        .collect();
    if name.chars().any(|c| c != '_') {
        return name;
    }
    let number = profile.id.strip_prefix(ID_PREFIX).unwrap_or("0");
    format!("Player{number}")
}

impl Profiles {
    /// The profiles of `root`. A missing or unreadable index means no extra
    /// profiles (and so no change to how the game starts); entries that are not
    /// valid are dropped, and an active profile that no longer exists is unset.
    pub fn load(root: &Path) -> Self {
        let mut index: Index = fs::read_to_string(root.join(INDEX_FILE))
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        let mut seen = Vec::new();
        index.profiles.retain(|p| {
            let keep = is_extra_id(&p.id) && is_stored_name(&p.name) && !seen.contains(&p.id);
            if keep {
                seen.push(p.id.clone());
            }
            keep
        });
        if index
            .active
            .as_deref()
            .is_some_and(|a| a != DEFAULT_ID && !seen.iter().any(|id| id == a))
        {
            index.active = None;
        }
        Self {
            root: root.to_owned(),
            index,
        }
    }

    /// Every profile, the built-in one first.
    pub fn list(&self) -> Vec<Profile> {
        std::iter::once(Profile::builtin())
            .chain(self.index.profiles.iter().cloned())
            .collect()
    }

    /// The profile the game starts in.
    pub fn active(&self) -> Profile {
        self.index
            .active
            .as_deref()
            .and_then(|id| self.index.profiles.iter().find(|p| p.id == id))
            .cloned()
            .unwrap_or_else(Profile::builtin)
    }

    /// The game folder of `profile`.
    pub fn dir(&self, profile: &Profile) -> PathBuf {
        if profile.is_default() {
            self.root.clone()
        } else {
            self.root.join(DIR).join(&profile.id)
        }
    }

    /// The name the profile with this id plays under, or `None` for the built-in one (it keeps
    /// its offline name) and for an id that is not in the index.
    pub fn player_name_of(&self, id: &str) -> Option<String> {
        self.index
            .profiles
            .iter()
            .find(|p| p.id == id)
            .map(player_name)
    }

    /// The name this run of the game plays under when the player has not given one: the running
    /// profile's, as the index has it now (a rename counts at once).
    pub fn running_player_name(root: &Path) -> Option<String> {
        Self::load(root).player_name_of(&running())
    }

    /// The folder to enter before the game starts, or `None` when the game
    /// stays in its own folder (no extra profile is active).
    pub fn active_dir(&self) -> Option<PathBuf> {
        let active = self.active();
        (!active.is_default()).then(|| self.dir(&active))
    }

    /// A new profile, with its folder, saved in the index. The name is only a
    /// label: it never becomes a path.
    pub fn create(&mut self, name: &str) -> io::Result<Profile> {
        let name = clean_name(name).ok_or_else(|| {
            invalid("the name must be 1 to 16 letters, numbers or symbols, without spaces")
        })?;
        if self.name_taken(&name, None) {
            return Err(invalid("there is already a profile with that name"));
        }
        let number = (1u32..)
            .find(|n| {
                let id = format!("{ID_PREFIX}{n}");
                !self.index.profiles.iter().any(|p| p.id == id)
            })
            .unwrap_or(1);
        let profile = Profile {
            id: format!("{ID_PREFIX}{number}"),
            name,
        };
        fs::create_dir_all(self.dir(&profile))?;
        self.index.profiles.push(profile.clone());
        self.save()?;
        Ok(profile)
    }

    /// Gives a profile another name. The built-in one keeps its own.
    pub fn rename(&mut self, id: &str, name: &str) -> io::Result<()> {
        let name = clean_name(name).ok_or_else(|| {
            invalid("the name must be 1 to 16 letters, numbers or symbols, without spaces")
        })?;
        if self.name_taken(&name, Some(id)) {
            return Err(invalid("there is already a profile with that name"));
        }
        let profile = self
            .index
            .profiles
            .iter_mut()
            .find(|p| p.id == id)
            .ok_or_else(|| invalid("no such profile"))?;
        profile.name = name;
        self.save()
    }

    /// Makes `id` the profile the game starts in next time.
    pub fn set_active(&mut self, id: &str) -> io::Result<()> {
        if id != DEFAULT_ID && !self.index.profiles.iter().any(|p| p.id == id) {
            return Err(invalid("no such profile"));
        }
        self.index.active = (id != DEFAULT_ID).then(|| id.to_owned());
        self.save()
    }

    /// Removes a profile from the index and deletes its folder, with its saves.
    /// The built-in profile can't be deleted. When the deleted profile was the
    /// active one, the built-in profile becomes active.
    pub fn delete(&mut self, id: &str) -> io::Result<()> {
        let at = self
            .index
            .profiles
            .iter()
            .position(|p| p.id == id)
            .ok_or_else(|| invalid("no such profile"))?;
        let profile = self.index.profiles.remove(at);
        if self.index.active.as_deref() == Some(id) {
            self.index.active = None;
        }
        // The index goes first: a folder left behind is only wasted space, while
        // an entry whose folder is gone would just be created empty again.
        self.save()?;
        match fs::remove_dir_all(self.dir(&profile)) {
            Err(e) if e.kind() != io::ErrorKind::NotFound => Err(e),
            _ => Ok(()),
        }
    }

    fn name_taken(&self, name: &str, except: Option<&str>) -> bool {
        name.eq_ignore_ascii_case(DEFAULT_NAME)
            || self
                .index
                .profiles
                .iter()
                .any(|p| Some(p.id.as_str()) != except && p.name.eq_ignore_ascii_case(name))
    }

    /// Writes the index through a temporary file, so a crash leaves the old one.
    fn save(&self) -> io::Result<()> {
        let text = serde_json::to_string_pretty(&self.index).map_err(io::Error::other)?;
        fs::create_dir_all(&self.root)?;
        let path = self.root.join(INDEX_FILE);
        let tmp = self.root.join(format!("{INDEX_FILE}.tmp"));
        fs::write(&tmp, text)?;
        fs::rename(&tmp, &path)
    }
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// A fresh empty folder under the system's temporary one.
    fn temp_root() -> PathBuf {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "horizon-profile-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn no_index_changes_nothing() {
        let root = temp_root();
        let profiles = Profiles::load(&root);
        assert_eq!(profiles.list(), vec![Profile::builtin()]);
        assert!(profiles.active().is_default());
        assert_eq!(profiles.active_dir(), None);
        // Loading never writes anything.
        assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    }

    #[test]
    fn broken_index_is_ignored() {
        let root = temp_root();
        fs::write(root.join(INDEX_FILE), "{ not json").unwrap();
        let profiles = Profiles::load(&root);
        assert_eq!(profiles.list().len(), 1);
        assert_eq!(profiles.active_dir(), None);
    }

    #[test]
    fn create_makes_the_folder_and_the_index() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let a = profiles.create("  Ana ").unwrap();
        let b = profiles.create("Beto").unwrap();
        assert_eq!((a.id.as_str(), a.name.as_str()), ("profile-1", "Ana"));
        assert_eq!(b.id, "profile-2");
        assert!(root.join("profiles/profile-1").is_dir());
        // A second start sees the same profiles.
        let again = Profiles::load(&root);
        assert_eq!(again.list().len(), 3);
        assert_eq!(again.list()[1], a);
    }

    #[test]
    fn names_are_checked() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        assert!(profiles.create("   ").is_err());
        assert!(profiles.create("a\nb").is_err());
        assert!(profiles.create(&"x".repeat(MAX_NAME_CHARS + 1)).is_err());
        assert!(profiles.create(&"x".repeat(MAX_NAME_CHARS)).is_ok());
        assert!(profiles.create("default").is_err());
        profiles.create("Ana").unwrap();
        assert!(profiles.create("ANA").is_err());
        // A name is a label, never a path.
        let p = profiles.create("../../etc").unwrap();
        assert!(profiles.dir(&p).starts_with(root.join(DIR)));
        assert!(profiles.dir(&p).ends_with(&p.id));
    }

    #[test]
    fn the_active_profile_decides_the_folder() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let ana = profiles.create("Ana").unwrap();
        assert_eq!(profiles.active_dir(), None);
        profiles.set_active(&ana.id).unwrap();
        let again = Profiles::load(&root);
        assert_eq!(again.active(), ana);
        assert_eq!(
            again.active_dir(),
            Some(root.join("profiles").join(&ana.id))
        );
        // Back to the built-in profile: the game folder itself, no change.
        let mut again = again;
        again.set_active(DEFAULT_ID).unwrap();
        assert_eq!(Profiles::load(&root).active_dir(), None);
        assert!(again.set_active("profile-99").is_err());
    }

    #[test]
    fn delete_removes_folder_and_resets_active() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let ana = profiles.create("Ana").unwrap();
        profiles.set_active(&ana.id).unwrap();
        fs::write(profiles.dir(&ana).join("options.txt"), "x").unwrap();
        profiles.delete(&ana.id).unwrap();
        assert!(!root.join("profiles/profile-1").exists());
        assert!(profiles.active().is_default());
        assert!(profiles.delete(DEFAULT_ID).is_err());
        assert!(profiles.delete(&ana.id).is_err());
        // The id is free again.
        assert_eq!(profiles.create("Otra").unwrap().id, "profile-1");
    }

    #[test]
    fn rename_keeps_the_folder() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let ana = profiles.create("Ana").unwrap();
        profiles.create("Beto").unwrap();
        profiles.rename(&ana.id, "Anita").unwrap();
        assert!(profiles.rename(&ana.id, "beto").is_err());
        assert!(profiles.rename(DEFAULT_ID, "Otro").is_err());
        let again = Profiles::load(&root);
        assert_eq!(again.list()[1].name, "Anita");
        assert_eq!(again.list()[1].id, ana.id);
    }

    #[test]
    fn a_hand_edited_index_cannot_leave_the_profiles_folder() {
        let root = temp_root();
        let index = r#"{
            "active": "../../x",
            "profiles": [
                {"id": "../../x", "name": "Evil"},
                {"id": "profile-01", "name": "Zero"},
                {"id": "profile-3", "name": "Ok"},
                {"id": "profile-3", "name": "Twice"},
                {"id": "profile-4", "name": ""}
            ]
        }"#;
        fs::write(root.join(INDEX_FILE), index).unwrap();
        let profiles = Profiles::load(&root);
        let ids: Vec<_> = profiles.list().into_iter().map(|p| p.id).collect();
        assert_eq!(ids, ["default", "profile-3"]);
        assert!(profiles.active().is_default());
    }

    #[test]
    fn a_profile_plays_under_its_own_name() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        let ana = profiles.create("Ana").unwrap();
        assert_eq!(profiles.player_name_of(&ana.id).as_deref(), Some("Ana"));
        // The built-in profile keeps its generated offline name, and an unknown id has none.
        assert_eq!(profiles.player_name_of(DEFAULT_ID), None);
        assert_eq!(profiles.player_name_of("profile-9"), None);
        // A rename counts at once, and after a restart.
        profiles.rename(&ana.id, "Anita").unwrap();
        assert_eq!(profiles.player_name_of(&ana.id).as_deref(), Some("Anita"));
        let again = Profiles::load(&root);
        assert_eq!(again.player_name_of(&ana.id).as_deref(), Some("Anita"));
        // Another profile is another player.
        let beto = profiles.create("Beto").unwrap();
        assert_ne!(
            profiles.player_name_of(&beto.id),
            profiles.player_name_of(&ana.id)
        );
    }

    #[test]
    fn a_name_is_a_name_a_player_can_have() {
        let root = temp_root();
        let mut profiles = Profiles::load(&root);
        assert!(profiles.create("Ana Maria").is_err(), "no spaces");
        assert!(profiles.create("\u{f1}and\u{fa}").is_err(), "no accents");
        assert!(profiles.create("Ana_01").is_ok());
        assert!(profiles.create(&"x".repeat(17)).is_err(), "16 at most");
    }

    #[test]
    fn an_old_label_that_is_not_a_player_name_plays_under_the_nearest_one() {
        let root = temp_root();
        let index = r#"{"profiles": [
            {"id": "profile-1", "name": "Ana Maria"},
            {"id": "profile-2", "name": "\u65e5\u672c\u8a9e"},
            {"id": "profile-3", "name": "A very long profile name"},
            {"id": "profile-4", "name": "Beto"}
        ]}"#;
        fs::write(root.join(INDEX_FILE), index).unwrap();
        let profiles = Profiles::load(&root);
        // None is dropped, and each keeps a valid name, the same every time.
        assert_eq!(profiles.list().len(), 5);
        for (id, expected) in [
            ("profile-1", "Ana_Maria"),
            ("profile-2", "Player2"),
            ("profile-3", "A_very_long_prof"),
            ("profile-4", "Beto"),
        ] {
            let name = profiles.player_name_of(id).unwrap();
            assert_eq!(name, expected, "{id}");
            assert!(crate::offline_name::is_valid(&name), "{name}");
        }
    }

    #[test]
    fn a_missing_active_profile_is_unset() {
        let root = temp_root();
        fs::write(
            root.join(INDEX_FILE),
            r#"{"active": "profile-7", "profiles": []}"#,
        )
        .unwrap();
        assert_eq!(Profiles::load(&root).active_dir(), None);
    }
}
