use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::time::{Duration, Instant};

use poll_promise::Promise;

use nullspace_structs::fragment::Attachment;
use nullspace_structs::profile::UserProfile;
use nullspace_structs::username::UserName;

use crate::promises::flatten_rpc;
use crate::rpc::get_rpc;

const PROFILE_RETRY_BACKOFF: Duration = Duration::from_secs(60);

#[derive(Default)]
pub struct ProfileLoader {
    entries: HashMap<UserName, ProfileEntry>,
    label_counts: HashMap<String, usize>,
    label_index_dirty: bool,
}

#[derive(Default)]
struct ProfileEntry {
    last_good: Option<UserProfile>,
    inflight: Option<Promise<Result<Option<UserProfile>, String>>>,
    last_error: Option<String>,
    retry_after: Option<Instant>,
    missing: bool,
    force_refresh: bool,
}

#[derive(Clone, Debug)]
pub struct ProfileView {
    pub display_name: Option<String>,
    pub avatar: Option<Attachment>,
    pub loaded: bool,
}

#[derive(Clone, Debug)]
pub struct DisplayLabel {
    pub display: String,
}

impl ProfileLoader {
    pub fn view(&mut self, username: &UserName) -> ProfileView {
        let entry = match self.entries.entry(username.clone()) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(entry) => {
                self.label_index_dirty = true;
                entry.insert(ProfileEntry::default())
            }
        };

        if let Some(promise) = entry.inflight.take() {
            let previous_display = entry
                .last_good
                .as_ref()
                .and_then(|profile| profile.display_name.clone());
            match promise.try_take() {
                Ok(result) => match result {
                    Ok(profile) => {
                        entry.missing = profile.is_none();
                        entry.last_good = profile;
                        entry.last_error = None;
                        entry.retry_after = None;
                        let next_display = entry
                            .last_good
                            .as_ref()
                            .and_then(|profile| profile.display_name.clone());
                        if previous_display != next_display {
                            self.label_index_dirty = true;
                        }
                    }
                    Err(err) => {
                        entry.last_error = Some(err);
                        entry.retry_after = Some(Instant::now() + PROFILE_RETRY_BACKOFF);
                    }
                },
                Err(promise) => {
                    entry.inflight = Some(promise);
                }
            }
        }

        let should_fetch = entry.inflight.is_none()
            && (entry.force_refresh
                || (entry.last_good.is_none()
                    && !entry.missing
                    && entry
                        .retry_after
                        .map(|when| when <= Instant::now())
                        .unwrap_or(true)));

        if should_fetch {
            entry.force_refresh = false;
            let username = username.clone();
            let promise = Promise::spawn_async(async move {
                flatten_rpc(get_rpc().user_profile(username).await)
            });
            entry.inflight = Some(promise);
        }

        let loaded = entry.last_good.is_some() || entry.missing;
        ProfileView {
            display_name: entry
                .last_good
                .as_ref()
                .and_then(|profile| profile.display_name.clone()),
            avatar: entry
                .last_good
                .as_ref()
                .and_then(|profile| profile.avatar.clone()),
            loaded,
        }
    }

    fn refresh_label_index(&mut self) {
        if !self.label_index_dirty {
            return;
        }

        self.label_counts.clear();
        for (entry_username, entry) in &self.entries {
            let base = entry
                .last_good
                .as_ref()
                .and_then(|profile| profile.display_name.clone())
                .unwrap_or_else(|| entry_username.as_str().to_string());
            self.label_counts
                .entry(base)
                .and_modify(|count| *count += 1)
                .or_insert(1);
        }

        self.label_index_dirty = false;
    }

    pub fn label_for(&mut self, username: &UserName) -> DisplayLabel {
        let view = self.view(username);
        self.refresh_label_index();

        let base = view
            .display_name
            .clone()
            .unwrap_or_else(|| username.as_str().to_string());
        let display = if view.display_name.is_some()
            && self.label_counts.get(&base).copied().unwrap_or(0) > 1
        {
            format!("{base} ({})", username.as_str())
        } else {
            base
        };
        DisplayLabel { display }
    }

    pub fn invalidate(&mut self, username: &UserName) {
        let entry = self.entries.entry(username.clone()).or_default();
        entry.missing = false;
        entry.last_error = None;
        entry.retry_after = None;
        entry.force_refresh = true;
    }
}
