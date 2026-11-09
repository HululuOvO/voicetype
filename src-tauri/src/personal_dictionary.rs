use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::path::Path;

use regex::{Captures, Regex};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_plugin_dialog::DialogExt;

const FILE_NAME: &str = "personal-dictionary.json";
const MAX_FILE_BYTES: u64 = 128 * 1024;
const MAX_ENTRIES: usize = 128;
const MAX_PHRASE_CHARS: usize = 120;

#[derive(Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub(crate) struct PersonalDictionary {
    hotwords: Vec<String>,
    replacements: Vec<Replacement>,
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Replacement {
    from: String,
    to: String,
}

impl PersonalDictionary {
    fn load(data_dir: &Path) -> Self {
        Self::read(&data_dir.join(FILE_NAME)).unwrap_or_else(|_| {
            log::warn!("Could not read personal dictionary; continuing without it");
            Self::default()
        })
    }

    fn read(path: &Path) -> Result<Self, String> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error.to_string()),
        };
        let mut content = String::new();
        let bytes = file
            .take(MAX_FILE_BYTES + 1)
            .read_to_string(&mut content)
            .map_err(|error| error.to_string())?;
        if bytes as u64 > MAX_FILE_BYTES {
            return Err("Dictionary exceeds 128 KiB".to_string());
        }
        serde_json::from_str(content.trim_start_matches('\u{feff}'))
            .map_err(|error| error.to_string())
    }

    fn write(&self, path: &Path) -> Result<(), String> {
        if self.hotwords.len() > MAX_ENTRIES || self.replacements.len() > MAX_ENTRIES {
            return Err("Up to 128 hotwords and 128 replacements can be saved".to_string());
        }
        if self.hotwords.iter().any(|word| !valid_hotword(word))
            || self.replacements.iter().any(|rule| {
                !valid_phrase(&rule.from) || !valid_phrase(&rule.to) || rule.from.contains('→')
            })
        {
            return Err("One or more dictionary entries are invalid".to_string());
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|error| error.to_string())?;
        if bytes.len() as u64 > MAX_FILE_BYTES {
            return Err("Dictionary exceeds 128 KiB".to_string());
        }
        let parent = path.parent().ok_or("Missing dictionary directory")?;
        let temporary = parent.join(format!(".voicepaste-dictionary-{}.tmp", uuid::Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut options = OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary)?;
            file.write_all(&bytes)?;
            file.sync_all()?;
            drop(file);
            fs::rename(&temporary, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result.map_err(|error| error.to_string())
    }

    fn correct<'text>(&self, text: &'text str) -> Cow<'text, str> {
        let mut rules = HashMap::new();
        for rule in self.replacements.iter().take(MAX_ENTRIES) {
            let from = rule.from.trim();
            if valid_phrase(from) && valid_phrase(rule.to.trim()) && from != rule.to {
                rules.entry(from).or_insert(rule.to.as_str());
            }
        }
        if rules.is_empty() {
            return Cow::Borrowed(text);
        }

        let mut sources: Vec<&str> = rules.keys().copied().collect();
        // Prefer the longest rule when two rules begin at the same position.
        sources.sort_unstable_by(|left, right| right.len().cmp(&left.len()).then(left.cmp(right)));
        let pattern = sources
            .into_iter()
            .map(regex::escape)
            .collect::<Vec<_>>()
            .join("|");
        let Ok(matcher) = Regex::new(&pattern) else {
            log::warn!("Could not compile personal dictionary replacement rules");
            return Cow::Borrowed(text);
        };

        // Each match is taken from the original text. Inserted words never trigger another rule.
        matcher.replace_all(text, |captures: &Captures<'_>| {
            let found = captures.get(0).expect("whole regex match");
            let phrase = found.as_str();
            let left = text[..found.start()].chars().next_back();
            let right = text[found.end()..].chars().next();
            let touches_left = phrase.chars().next().is_some_and(ascii_word_char)
                && left.is_some_and(ascii_word_char);
            let touches_right = phrase.chars().next_back().is_some_and(ascii_word_char)
                && right.is_some_and(ascii_word_char);
            if touches_left || touches_right {
                phrase.to_owned()
            } else {
                rules.get(phrase).copied().unwrap_or(phrase).to_owned()
            }
        })
    }
}

fn valid_phrase(phrase: &str) -> bool {
    !phrase.is_empty()
        && phrase.chars().count() <= MAX_PHRASE_CHARS
        && !phrase.chars().any(char::is_control)
}

fn valid_hotword(word: &str) -> bool {
    valid_phrase(word) && !word.contains([',', '，', '|'])
}

fn ascii_word_char(character: char) -> bool {
    character.is_ascii_alphanumeric() || character == '_'
}

fn hotword_key(term: &str) -> String {
    term.split('|')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

pub(crate) fn with_hotwords(data_dir: &Path, original: &[String]) -> Vec<String> {
    let mut words = original.to_vec();
    let mut seen: HashSet<_> = original.iter().map(|term| hotword_key(term)).collect();
    for term in PersonalDictionary::load(data_dir)
        .hotwords
        .into_iter()
        .take(MAX_ENTRIES)
    {
        let term = term.trim();
        if valid_hotword(term) && seen.insert(hotword_key(term)) {
            words.push(term.to_owned());
        }
    }
    words
}

fn dictionary_path(app: &AppHandle) -> Result<std::path::PathBuf, String> {
    app.path()
        .app_data_dir()
        .map(|dir| dir.join(FILE_NAME))
        .map_err(|error| error.to_string())
}

#[tauri::command]
pub(crate) async fn load_personal_dictionary(app: AppHandle) -> Result<PersonalDictionary, String> {
    PersonalDictionary::read(&dictionary_path(&app)?)
}

#[tauri::command]
pub(crate) async fn save_personal_dictionary(app: AppHandle, data: PersonalDictionary) -> Result<(), String> {
    data.write(&dictionary_path(&app)?)
}

#[tauri::command]
pub(crate) async fn export_personal_dictionary(app: AppHandle, data: PersonalDictionary) -> Result<bool, String> {
    let selected = app.dialog().file().add_filter("JSON", &["json"])
        .set_file_name(FILE_NAME).blocking_save_file();
    let Some(selected) = selected else { return Ok(false) };
    let path = selected.into_path().map_err(|error| error.to_string())?;
    data.write(&path)?;
    Ok(true)
}

#[tauri::command]
pub(crate) async fn import_personal_dictionary(app: AppHandle) -> Result<Option<PersonalDictionary>, String> {
    let selected = app.dialog().file().add_filter("JSON", &["json"]).blocking_pick_file();
    let Some(selected) = selected else { return Ok(None) };
    let path = selected.into_path().map_err(|error| error.to_string())?;
    PersonalDictionary::read(&path).map(Some)
}

pub(crate) fn correct_text<'text>(data_dir: Option<&Path>, text: &'text str) -> Cow<'text, str> {
    match data_dir {
        Some(path) => PersonalDictionary::load(path).correct(text),
        None => Cow::Borrowed(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrections_apply_once_and_prefer_the_longest_rule() {
        let dictionary: PersonalDictionary = serde_json::from_str(
            r#"{"replacements":[{"from":"林小晨","to":"林晓晨"},{"from":"林小晨同学","to":"林晓晨同学"},{"from":"林晓晨同学","to":"另一人"}]}"#,
        ).unwrap();
        assert_eq!(dictionary.correct("请联系林小晨同学和林小晨"), "请联系林晓晨同学和林晓晨");
    }

    #[test]
    fn english_rules_do_not_replace_parts_of_ascii_words() {
        let dictionary: PersonalDictionary = serde_json::from_str(
            r#"{"replacements":[{"from":"obsidian","to":"Obsidian"}]}"#,
        ).unwrap();
        assert_eq!(dictionary.correct("打开obsidian和obsidianite"), "打开Obsidian和obsidianite");
    }

    #[test]
    fn keeps_existing_hotword_weight_and_adds_private_terms() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join(FILE_NAME), r#"{"hotwords":["Obsidian","林晓晨", "Obsidian"]}"#).unwrap();
        let original = vec!["obsidian|8".to_string()];
        assert_eq!(with_hotwords(folder.path(), &original), vec!["obsidian|8", "林晓晨"]);
    }

    #[test]
    fn malformed_dictionary_leaves_text_unchanged() {
        let folder = tempfile::tempdir().unwrap();
        std::fs::write(folder.path().join(FILE_NAME), "{").unwrap();
        assert_eq!(correct_text(Some(folder.path()), "原文"), "原文");
    }
}
