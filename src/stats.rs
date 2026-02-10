// SPDX-FileCopyrightText: 2026 Sébastien Helleu <flashcode@flashtux.org>
//
// SPDX-License-Identifier: GPL-3.0-or-later

//! Statistics for PO files.

use std::fs::File;
use std::io::Read;
use std::ops::AddAssign;
use std::path::{Path, PathBuf};

use colored::Colorize;
use rayon::prelude::*;
use serde::Serialize;

use crate::args;
use crate::dir::find_po_files;
use crate::po::parser::Parser;
use crate::words::{CharPos, WordPos};

#[derive(Clone, Copy, Default, Serialize)]
struct Entries {
    total: u64,
    translated: u64,
    fuzzy: u64,
    untranslated: u64,
    obsolete: u64,
}

#[derive(Clone, Copy, Default, Serialize)]
struct Counts {
    id_total: u64,
    id_translated: u64,
    id_fuzzy: u64,
    id_untranslated: u64,
    id_obsolete: u64,
    str_translated: u64,
    str_fuzzy: u64,
    str_untranslated: u64, // Always 0, kept for symmetry.
    str_obsolete: u64,
}

#[derive(Default, Serialize)]
struct StatsFile {
    path: PathBuf,
    entries: Entries,
    #[serde(skip_serializing_if = "Option::is_none")]
    words: Option<Counts>,
    #[serde(skip_serializing_if = "Option::is_none")]
    chars: Option<Counts>,
}

impl std::fmt::Display for Entries {
    /// Format the `Entries` struct for display, showing a progress bar and statistics.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let (pct_translated, pct_fuzzy, pct_untranslated, pct_obsolete) = self.pct();
        let chars_translated = (pct_translated / 5) as usize;
        let chars_fuzzy = (pct_fuzzy / 5) as usize;
        let chars_untranslated = (pct_untranslated / 5) as usize;
        let chars_obsolete = 20 - chars_translated - chars_fuzzy - chars_untranslated;
        let mut bar = String::new();
        if self.translated == self.total {
            // If all entries are translated, make it more visible.
            bar.push_str("█".repeat(chars_translated).green().to_string().as_str());
        } else {
            bar.push_str(
                "█"
                    .repeat(chars_translated)
                    .green()
                    .dimmed()
                    .to_string()
                    .as_str(),
            );
        }
        bar.push_str(
            "▒"
                .repeat(chars_fuzzy)
                .yellow()
                .dimmed()
                .to_string()
                .as_str(),
        );
        bar.push_str(" ".repeat(chars_untranslated).red().to_string().as_str());
        bar.push_str(" ".repeat(chars_obsolete).magenta().to_string().as_str());
        write!(
            f,
            "{}{}{} {} = {} {} + {} {} + {} {} + {} {}",
            "[".dimmed(),
            bar,
            "]".dimmed(),
            self.total,
            format!("{}", self.translated).bright_green(),
            format!("({pct_translated}%)").green(),
            format!("{}", self.fuzzy).bright_yellow(),
            format!("({pct_fuzzy}%)").yellow(),
            format!("{}", self.untranslated).bright_red(),
            format!("({pct_untranslated}%)").red(),
            format!("{}", self.obsolete).bright_magenta(),
            format!("({pct_obsolete}%)").magenta(),
        )
    }
}

impl AddAssign for Entries {
    /// Add the values from another `Entries` struct to this one.
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            total: self.total + other.total,
            translated: self.translated + other.translated,
            fuzzy: self.fuzzy + other.fuzzy,
            untranslated: self.untranslated + other.untranslated,
            obsolete: self.obsolete + other.obsolete,
        };
    }
}

impl Entries {
    /// Return the percentage of translated entries as integer.
    pub fn pct_translated(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.translated * 100) / self.total
        }
    }

    /// Return the ratio of translated entries, scaled to 1,000,000.
    pub fn ratio_translated(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.translated * 1_000_000) / self.total
        }
    }

    /// Return the percentage of fuzzy entries as integer.
    pub fn pct_fuzzy(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.fuzzy * 100) / self.total
        }
    }

    /// Return the ratio of fuzzy entries, scaled to 1,000,000.
    pub fn ratio_fuzzy(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.fuzzy * 1_000_000) / self.total
        }
    }

    /// Return the percentage of untranslated entries as integer.
    pub fn pct_untranslated(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.untranslated * 100) / self.total
        }
    }

    /// Return the ratio of untranslated entries, scaled to 1,000,000.
    pub fn ratio_untranslated(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.untranslated * 1_000_000) / self.total
        }
    }

    /// Return the percentage of obsolete entries as integer.
    pub fn pct_obsolete(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.obsolete * 100) / self.total
        }
    }

    /// Return the ratio of obsolete entries, scaled to 1,000,000.
    pub fn ratio_obsolete(&self) -> u64 {
        if self.total == 0 {
            0
        } else {
            (self.obsolete * 1_000_000) / self.total
        }
    }

    /// Return a tuple of (translated, fuzzy, untranslated, obsolete) percentages as integers.
    pub fn pct(&self) -> (u64, u64, u64, u64) {
        (
            self.pct_translated(),
            self.pct_fuzzy(),
            self.pct_untranslated(),
            self.pct_obsolete(),
        )
    }
}

impl AddAssign for Counts {
    /// Add the values from another `Counts` struct to this one.
    fn add_assign(&mut self, other: Self) {
        *self = Self {
            id_total: self.id_total + other.id_total,
            id_translated: self.id_translated + other.id_translated,
            id_fuzzy: self.id_fuzzy + other.id_fuzzy,
            id_untranslated: self.id_untranslated + other.id_untranslated,
            id_obsolete: self.id_obsolete + other.id_obsolete,
            str_translated: self.str_translated + other.str_translated,
            str_fuzzy: self.str_fuzzy + other.str_fuzzy,
            str_untranslated: self.str_untranslated + other.str_untranslated,
            str_obsolete: self.str_obsolete + other.str_obsolete,
        };
    }
}

impl Counts {
    /// Return the percentage of translated words/characters in msgid as integer.
    pub fn pct_id_translated(&self) -> u64 {
        if self.id_total == 0 {
            0
        } else {
            (self.id_translated * 100) / self.id_total
        }
    }

    /// Return the percentage of fuzzy words/characters in msgid as integer.
    pub fn pct_id_fuzzy(&self) -> u64 {
        if self.id_total == 0 {
            0
        } else {
            (self.id_fuzzy * 100) / self.id_total
        }
    }

    /// Return the percentage of untranslated words/characters in msgid as integer.
    pub fn pct_id_untranslated(&self) -> u64 {
        if self.id_total == 0 {
            0
        } else {
            (self.id_untranslated * 100) / self.id_total
        }
    }

    /// Return the percentage of obsolete words/characters in msgid as integer.
    pub fn pct_id_obsolete(&self) -> u64 {
        if self.id_total == 0 {
            0
        } else {
            (self.id_obsolete * 100) / self.id_total
        }
    }
}

impl std::fmt::Display for StatsFile {
    /// Format the `StatsFile` struct for display, showing the file path and entry statistics.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.entries)
    }
}

impl StatsFile {
    #[must_use]
    /// Create a new `StatsFile` for the given path.
    pub fn new(path: &Path) -> Self {
        Self {
            path: PathBuf::from(path),
            ..Default::default()
        }
    }

    /// Return a formatted string with colors for translated words/characters statistics.
    fn to_string_words_translated(&self) -> String {
        if let Some(words) = &self.words
            && let Some(chars) = &self.chars
        {
            format!(
                "{:<14} {} {} {} {} {} {} {} {}",
                "Translated".bright_green(),
                format!("{:10}", self.entries.translated).bright_green(),
                format!("({:3}%)", self.entries.pct_translated()).green(),
                format!("{:10}", words.id_translated).bright_green(),
                format!("({:3}%)", words.pct_id_translated()).green(),
                format!("{:10}", words.str_translated).bright_green(),
                format!("{:10}", chars.id_translated).bright_green(),
                format!("({:3}%)", chars.pct_id_translated()).green(),
                format!("{:10}", chars.str_translated).bright_green(),
            )
        } else {
            String::new()
        }
    }

    /// Return a formatted string with colors for fuzzy words/chars statistics.
    fn to_string_words_fuzzy(&self) -> String {
        if let Some(words) = &self.words
            && let Some(chars) = &self.chars
        {
            format!(
                "{:<14} {} {} {} {} {} {} {} {}",
                "Fuzzy".yellow(),
                format!("{:10}", self.entries.fuzzy).bright_yellow(),
                format!("({:3}%)", self.entries.pct_fuzzy()).yellow(),
                format!("{:10}", words.id_fuzzy).bright_yellow(),
                format!("({:3}%)", words.pct_id_fuzzy()).yellow(),
                format!("{:10}", words.str_fuzzy).bright_yellow(),
                format!("{:10}", chars.id_fuzzy).bright_yellow(),
                format!("({:3}%)", chars.pct_id_fuzzy()).yellow(),
                format!("{:10}", chars.str_fuzzy).bright_yellow(),
            )
        } else {
            String::new()
        }
    }

    /// Return a formatted string with colors for untranslated words/characters statistics.
    fn to_string_words_untranslated(&self) -> String {
        if let Some(words) = &self.words
            && let Some(chars) = &self.chars
        {
            format!(
                "{:<14} {} {} {} {} {} {} {} {}",
                "Untranslated".bright_red(),
                format!("{:10}", self.entries.untranslated).bright_red(),
                format!("({:3}%)", self.entries.pct_untranslated()).red(),
                format!("{:10}", words.id_untranslated).bright_red(),
                format!("({:3}%)", words.pct_id_untranslated()).red(),
                format!("{:>10}", words.str_untranslated).red(),
                format!("{:10}", chars.id_untranslated).bright_red(),
                format!("({:3}%)", chars.pct_id_untranslated()).red(),
                format!("{:>10}", chars.str_untranslated).red(),
            )
        } else {
            String::new()
        }
    }

    /// Return a formatted string with colors for obsolete words/characters statistics.
    fn to_string_words_obsolete(&self) -> String {
        if let Some(words) = &self.words
            && let Some(chars) = &self.chars
        {
            format!(
                "{:<14} {} {} {} {} {} {} {} {}",
                "Obsolete".bright_magenta(),
                format!("{:10}", self.entries.obsolete).bright_magenta(),
                format!("({:3}%)", self.entries.pct_obsolete()).magenta(),
                format!("{:10}", words.id_obsolete).bright_magenta(),
                format!("({:3}%)", words.pct_id_obsolete()).magenta(),
                format!("{:10}", words.str_obsolete).bright_magenta(),
                format!("{:10}", chars.id_obsolete).bright_magenta(),
                format!("({:3}%)", chars.pct_id_obsolete()).magenta(),
                format!("{:10}", chars.str_obsolete).bright_magenta(),
            )
        } else {
            String::new()
        }
    }

    /// Return a formatted string with colors for total words/characters statistics.
    fn to_string_words_total(&self) -> String {
        if let Some(words) = &self.words
            && let Some(chars) = &self.chars
        {
            format!(
                "{:<10}    {:11}       {:11}       {:11}{:11}       {:11}",
                "Total".bright_white(),
                self.entries.total,
                words.id_total,
                words.str_translated,
                chars.id_total,
                chars.str_translated,
            )
        } else {
            String::new()
        }
    }

    /// Return a formatted string with colors for all word/characters statistics.
    pub fn to_string_words(&self) -> String {
        format!(
            "                    Entries          \
            Words (src / translated)     \
            Chars (src / translated)\n\
            {}\n{}\n{}\n{}\n{}",
            self.to_string_words_translated(),
            self.to_string_words_fuzzy(),
            self.to_string_words_untranslated(),
            self.to_string_words_obsolete(),
            self.to_string_words_total(),
        )
    }
}

/// Count words in a given string.
fn count_words(s: &str, format: &str) -> u64 {
    WordPos::new(s, format).count() as u64
}

/// Count characters (non-whitespace or punctuation) in a given string.
fn count_chars(s: &str, format: &str) -> u64 {
    CharPos::new(s, format).count() as u64
}

/// Compute statistics for a single PO file at the given path.
fn stats_file(path: &PathBuf, args: &args::StatsArgs) -> Result<StatsFile, std::io::Error> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    let _ = file.read_to_end(&mut buf)?;
    let parser = Parser::new(&buf);
    let mut stats = StatsFile::new(path.as_path());
    let mut words = Counts::default();
    let mut chars = Counts::default();
    for entry in parser {
        if entry.is_header() {
            continue;
        }
        let (words_id, chars_id) = if args.words
            && let Some(msgid) = &entry.msgid
        {
            (
                count_words(msgid.value.as_str(), &entry.format),
                count_chars(msgid.value.as_str(), &entry.format),
            )
        } else {
            (0, 0)
        };
        let (words_str, chars_str) = if args.words
            && let Some(msgstr) = entry.msgstr.get(&0)
        {
            (
                count_words(msgstr.value.as_str(), &entry.format),
                count_chars(msgstr.value.as_str(), &entry.format),
            )
        } else {
            (0, 0)
        };
        stats.entries.total += 1;
        words.id_total += words_id;
        chars.id_total += chars_id;
        if entry.fuzzy {
            stats.entries.fuzzy += 1;
            words.id_fuzzy += words_id;
            chars.id_fuzzy += chars_id;
            words.str_fuzzy += words_str;
            chars.str_fuzzy += chars_str;
        } else if entry.obsolete {
            stats.entries.obsolete += 1;
            words.id_obsolete += words_id;
            chars.id_obsolete += chars_id;
            words.str_obsolete += words_str;
            chars.str_obsolete += chars_str;
        } else if entry.is_translated() {
            stats.entries.translated += 1;
            words.id_translated += words_id;
            chars.id_translated += chars_id;
            words.str_translated += words_str;
            chars.str_translated += chars_str;
        } else {
            stats.entries.untranslated += 1;
            words.id_untranslated += words_id;
            chars.id_untranslated += chars_id;
        }
    }
    if args.words {
        stats.words = Some(words);
        stats.chars = Some(chars);
    }
    Ok(stats)
}

/// Compute total for statistics.
fn compute_total_stats(stats: &Vec<StatsFile>) -> StatsFile {
    let mut total = StatsFile::default();
    let mut words = Counts::default();
    let mut chars = Counts::default();
    let mut add_words = false;
    let mut add_chars = false;
    for stat in stats {
        total.entries += stat.entries;
        if let Some(stat_words) = &stat.words {
            words += *stat_words;
            add_words = true;
        }
        if let Some(stat_chars) = &stat.chars {
            chars += *stat_chars;
            add_chars = true;
        }
    }
    total.path = PathBuf::from(format!("Total ({})", stats.len()));
    if add_words {
        total.words = Some(words);
    }
    if add_chars {
        total.chars = Some(chars);
    }
    total
}

/// Display statistics for a list of PO files, formatted according to the arguments.
fn display_stats(stats: &Vec<StatsFile>, args: &args::StatsArgs) -> i32 {
    let path_max_len = stats
        .iter()
        .map(|s| s.path.as_os_str().len())
        .max()
        .unwrap_or(0);
    if args.words {
        match args.output {
            args::StatsOutputFormat::Human => {
                for (idx, stat) in stats.iter().enumerate() {
                    if idx > 0 {
                        println!();
                    }
                    println!("{}:\n{}", stat.path.display(), stat.to_string_words());
                }
            }
            args::StatsOutputFormat::Json => {
                println!("{}", serde_json::to_string(&stats).unwrap_or_default());
            }
        }
    } else {
        match args.output {
            args::StatsOutputFormat::Human => {
                for stat in stats {
                    println!(
                        "{:width$} {}",
                        stat.path.display(),
                        stat.entries,
                        width = path_max_len
                    );
                    if args.words {
                        println!("{}", stat.to_string_words());
                    }
                }
            }
            args::StatsOutputFormat::Json => {
                println!("{}", serde_json::to_string(&stats).unwrap_or_default());
            }
        }
    }
    0
}

/// Compute and display statistics for all PO files.
pub fn run_stats(args: &args::StatsArgs) -> i32 {
    let po_files = find_po_files(&args.files);
    let mut stats: Vec<StatsFile> = po_files
        .par_iter()
        .map(|f| {
            stats_file(f, args).map_err(|e| {
                eprintln!("Error processing file {}: {}", f.display(), e);
                e
            })
        })
        .filter_map(Result::ok)
        .collect();
    match args.sort {
        args::StatsSort::Path => {
            stats.sort_by(|a, b| a.path.cmp(&b.path));
        }
        args::StatsSort::Status => {
            stats.sort_by_key(|s| {
                (
                    u64::MAX - s.entries.ratio_translated(),
                    u64::MAX - s.entries.translated,
                    u64::MAX - s.entries.ratio_fuzzy(),
                    u64::MAX - s.entries.fuzzy,
                    u64::MAX - s.entries.ratio_untranslated(),
                    u64::MAX - s.entries.untranslated,
                    u64::MAX - s.entries.ratio_obsolete(),
                    u64::MAX - s.entries.obsolete,
                    s.path.clone(),
                )
            });
        }
    }
    if stats.len() > 1 {
        stats.push(compute_total_stats(&stats));
    }
    display_stats(&stats, args)
}
