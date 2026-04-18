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
use crate::po::format::language::Language;
use crate::po::format::{iter::FormatWordPos, strip_formats};
use crate::po::parser::Parser;

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
    pub const fn pct_translated(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.translated * 100) / self.total
    }

    /// Return the ratio of translated entries, scaled to 1,000,000.
    pub const fn ratio_translated(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.translated * 1_000_000) / self.total
    }

    /// Return the percentage of fuzzy entries as integer.
    pub const fn pct_fuzzy(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.fuzzy * 100) / self.total
    }

    /// Return the ratio of fuzzy entries, scaled to 1,000,000.
    pub const fn ratio_fuzzy(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.fuzzy * 1_000_000) / self.total
    }

    /// Return the percentage of untranslated entries as integer.
    pub const fn pct_untranslated(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.untranslated * 100) / self.total
    }

    /// Return the ratio of untranslated entries, scaled to 1,000,000.
    pub const fn ratio_untranslated(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.untranslated * 1_000_000) / self.total
    }

    /// Return the percentage of obsolete entries as integer.
    pub const fn pct_obsolete(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.obsolete * 100) / self.total
    }

    /// Return the ratio of obsolete entries, scaled to 1,000,000.
    pub const fn ratio_obsolete(&self) -> u64 {
        if self.total == 0 {
            return 0;
        }
        (self.obsolete * 1_000_000) / self.total
    }

    /// Return a tuple of (translated, fuzzy, untranslated, obsolete) percentages as integers.
    pub const fn pct(&self) -> (u64, u64, u64, u64) {
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
    pub const fn pct_id_translated(&self) -> u64 {
        if self.id_total == 0 {
            return 0;
        }
        (self.id_translated * 100) / self.id_total
    }

    /// Return the percentage of fuzzy words/characters in msgid as integer.
    pub const fn pct_id_fuzzy(&self) -> u64 {
        if self.id_total == 0 {
            return 0;
        }
        (self.id_fuzzy * 100) / self.id_total
    }

    /// Return the percentage of untranslated words/characters in msgid as integer.
    pub const fn pct_id_untranslated(&self) -> u64 {
        if self.id_total == 0 {
            return 0;
        }
        (self.id_untranslated * 100) / self.id_total
    }

    /// Return the percentage of obsolete words/characters in msgid as integer.
    pub const fn pct_id_obsolete(&self) -> u64 {
        if self.id_total == 0 {
            return 0;
        }
        (self.id_obsolete * 100) / self.id_total
    }
}

impl std::fmt::Display for StatsFile {
    /// Format the `StatsFile` struct for display, showing the file path and entry statistics.
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}: {}", self.path.display(), self.entries)
    }
}

impl StatsFile {
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
    fn to_string_words(&self) -> String {
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
fn count_words(s: &str) -> u64 {
    FormatWordPos::new(s, &Language::Null).count() as u64
}

/// Count characters (non-whitespace or punctuation) in a given string.
fn count_chars(s: &str) -> u64 {
    s.chars()
        .filter(|c| c.is_alphanumeric() || *c == '-')
        .count() as u64
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
            let stripped = strip_formats(&msgid.value, &entry.format_language);
            (count_words(&stripped), count_chars(&stripped))
        } else {
            (0, 0)
        };
        let (words_str, chars_str) = if args.words
            && let Some(msgstr) = entry.msgstr.get(&0)
        {
            let stripped = strip_formats(&msgstr.value, &entry.format_language);
            (count_words(&stripped), count_chars(&stripped))
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
        .map(|path| {
            stats_file(path, args).map_err(|e| {
                eprintln!("Error processing file {}: {}", path.display(), e);
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entries(
        total: u64,
        translated: u64,
        fuzzy: u64,
        untranslated: u64,
        obsolete: u64,
    ) -> Entries {
        Entries {
            total,
            translated,
            fuzzy,
            untranslated,
            obsolete,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_counts(
        id_total: u64,
        id_translated: u64,
        id_fuzzy: u64,
        id_untranslated: u64,
        id_obsolete: u64,
        str_translated: u64,
        str_fuzzy: u64,
        str_untranslated: u64,
        str_obsolete: u64,
    ) -> Counts {
        Counts {
            id_total,
            id_translated,
            id_fuzzy,
            id_untranslated,
            id_obsolete,
            str_translated,
            str_fuzzy,
            str_untranslated,
            str_obsolete,
        }
    }

    #[test]
    fn test_entries_pct_ratio() {
        let e = Entries::default();
        assert_eq!(e.pct_translated(), 0);
        assert_eq!(e.ratio_translated(), 0);
        assert_eq!(e.pct_fuzzy(), 0);
        assert_eq!(e.ratio_fuzzy(), 0);
        assert_eq!(e.pct_untranslated(), 0);
        assert_eq!(e.ratio_untranslated(), 0);
        assert_eq!(e.pct_obsolete(), 0);
        assert_eq!(e.ratio_obsolete(), 0);
        assert_eq!(e.pct(), (0, 0, 0, 0));

        let e = make_entries(0, 0, 0, 0, 0);
        assert_eq!(e.pct_translated(), 0);
        assert_eq!(e.ratio_translated(), 0);
        assert_eq!(e.pct_fuzzy(), 0);
        assert_eq!(e.ratio_fuzzy(), 0);
        assert_eq!(e.pct_untranslated(), 0);
        assert_eq!(e.ratio_untranslated(), 0);
        assert_eq!(e.pct_obsolete(), 0);
        assert_eq!(e.ratio_obsolete(), 0);
        assert_eq!(e.pct(), (0, 0, 0, 0));

        let e = make_entries(3, 1, 1, 1, 0);
        assert_eq!(e.pct_translated(), 33);
        assert_eq!(e.ratio_translated(), 333_333);
        assert_eq!(e.pct_fuzzy(), 33);
        assert_eq!(e.ratio_fuzzy(), 333_333);
        assert_eq!(e.pct_untranslated(), 33);
        assert_eq!(e.ratio_untranslated(), 333_333);

        let e = make_entries(200, 150, 30, 10, 10);
        assert_eq!(e.pct_translated(), 75);
        assert_eq!(e.ratio_translated(), 750_000);
        assert_eq!(e.pct_fuzzy(), 15);
        assert_eq!(e.ratio_fuzzy(), 150_000);
        assert_eq!(e.pct_untranslated(), 5);
        assert_eq!(e.ratio_untranslated(), 50_000);
        assert_eq!(e.pct_obsolete(), 5);
        assert_eq!(e.ratio_obsolete(), 50_000);
        assert_eq!(e.pct(), (75, 15, 5, 5));

        let e = make_entries(100, 100, 0, 0, 0);
        assert_eq!(e.pct_translated(), 100);
        assert_eq!(e.ratio_translated(), 1_000_000);
        assert_eq!(e.pct_fuzzy(), 0);
        assert_eq!(e.ratio_fuzzy(), 0);
        assert_eq!(e.pct_untranslated(), 0);
        assert_eq!(e.ratio_untranslated(), 0);
        assert_eq!(e.pct_obsolete(), 0);
        assert_eq!(e.ratio_obsolete(), 0);
        assert_eq!(e.pct(), (100, 0, 0, 0));
    }

    #[test]
    fn test_entries_add_assign() {
        let mut a = make_entries(10, 4, 3, 2, 1);
        let b = Entries::default();
        a += b;
        assert_eq!(a.total, 10);
        assert_eq!(a.translated, 4);
        assert_eq!(a.fuzzy, 3);
        assert_eq!(a.untranslated, 2);
        assert_eq!(a.obsolete, 1);
        let b = make_entries(20, 15, 3, 1, 1);
        a += b;
        assert_eq!(a.total, 30);
        assert_eq!(a.translated, 19);
        assert_eq!(a.fuzzy, 6);
        assert_eq!(a.untranslated, 3);
        assert_eq!(a.obsolete, 2);
    }

    #[test]
    fn test_entries_display() {
        let e = make_entries(100, 80, 10, 6, 4);
        let s = format!("{e}");
        assert!(!s.is_empty());
    }

    #[test]
    fn test_counts_pct_id_translated() {
        let c = Counts::default();
        assert_eq!(c.pct_id_translated(), 0);
        assert_eq!(c.pct_id_fuzzy(), 0);
        assert_eq!(c.pct_id_untranslated(), 0);
        assert_eq!(c.pct_id_obsolete(), 0);

        let c = make_counts(100, 60, 20, 10, 10, 50, 15, 0, 8);
        assert_eq!(c.pct_id_translated(), 60);
        assert_eq!(c.pct_id_fuzzy(), 20);
        assert_eq!(c.pct_id_untranslated(), 10);
        assert_eq!(c.pct_id_obsolete(), 10);
    }

    #[test]
    fn test_counts_add_assign() {
        let mut a = make_counts(50, 30, 10, 6, 4, 25, 8, 1, 3);
        let b = Counts::default();
        a += b;
        assert_eq!(a.id_total, 50);
        assert_eq!(a.id_translated, 30);
        assert_eq!(a.id_fuzzy, 10);
        assert_eq!(a.id_untranslated, 6);
        assert_eq!(a.id_obsolete, 4);
        assert_eq!(a.str_translated, 25);
        assert_eq!(a.str_fuzzy, 8);
        assert_eq!(a.str_untranslated, 1);
        assert_eq!(a.str_obsolete, 3);
        let b = make_counts(100, 60, 20, 10, 10, 50, 15, 0, 8);
        a += b;
        assert_eq!(a.id_total, 150);
        assert_eq!(a.id_translated, 90);
        assert_eq!(a.id_fuzzy, 30);
        assert_eq!(a.id_untranslated, 16);
        assert_eq!(a.id_obsolete, 14);
        assert_eq!(a.str_translated, 75);
        assert_eq!(a.str_fuzzy, 23);
        assert_eq!(a.str_untranslated, 1);
        assert_eq!(a.str_obsolete, 11);
    }

    #[test]
    fn test_stats_file_default() {
        let sf = StatsFile::default();
        assert_eq!(sf.path, PathBuf::new());
        assert_eq!(sf.entries.total, 0);
        assert!(sf.words.is_none());
        assert!(sf.chars.is_none());
    }

    #[test]
    fn test_stats_file_new() {
        let sf = StatsFile::new(Path::new("/tmp/fr.po"));
        assert_eq!(sf.path, PathBuf::from("/tmp/fr.po"));
        assert_eq!(sf.entries.total, 0);
        assert!(sf.words.is_none());
        assert!(sf.chars.is_none());
    }

    #[test]
    fn test_stats_file_display() {
        let mut sf = StatsFile::new(Path::new("fr.po"));
        sf.entries = make_entries(50, 40, 5, 3, 2);
        let s = format!("{sf}");
        assert!(s.contains("fr.po"));
    }

    #[test]
    fn test_stats_file_to_string_words_none() {
        let sf = StatsFile::new(Path::new("fr.po"));
        let s = sf.to_string_words();
        assert!(s.contains("Entries"));
        assert!(s.contains("Words"));
        assert!(s.contains("Chars"));
    }

    #[test]
    fn test_stats_file_to_string_words_some() {
        let mut sf = StatsFile::new(Path::new("fr.po"));
        sf.entries = make_entries(100, 80, 10, 5, 5);
        sf.words = Some(make_counts(500, 400, 50, 30, 20, 380, 45, 0, 18));
        sf.chars = Some(make_counts(3000, 2400, 300, 180, 120, 2300, 280, 0, 110));
        let s = sf.to_string_words();
        assert!(s.contains("Entries"));
        assert!(!s.is_empty());
    }

    #[test]
    fn test_count_words() {
        assert_eq!(count_words(""), 0);
        assert_eq!(count_words("hello"), 1);
        assert_eq!(count_words("hello, world!"), 2);
    }

    #[test]
    fn test_count_chars() {
        assert_eq!(count_chars(""), 0);
        assert_eq!(count_chars("hello"), 5);
        assert_eq!(count_chars("hello!"), 5);
        assert_eq!(count_chars("a b c"), 3);
    }

    #[test]
    fn test_compute_total_stats_empty() {
        let stats: Vec<StatsFile> = vec![];
        let total = compute_total_stats(&stats);
        assert_eq!(total.entries.total, 0);
        assert!(total.words.is_none());
        assert!(total.chars.is_none());
        assert!(total.path.display().to_string().contains("Total (0)"));
    }

    #[test]
    fn test_compute_total_stats_one_file() {
        let mut sf = StatsFile::new(Path::new("fr.po"));
        sf.entries = make_entries(10, 8, 1, 1, 0);
        let total = compute_total_stats(&vec![sf]);
        assert_eq!(total.entries.total, 10);
        assert_eq!(total.entries.translated, 8);
        assert_eq!(total.entries.fuzzy, 1);
        assert_eq!(total.entries.untranslated, 1);
        assert_eq!(total.entries.obsolete, 0);
        assert!(total.words.is_none());
        assert!(total.chars.is_none());
        assert!(total.path.display().to_string().contains("Total (1)"));
    }

    #[test]
    fn test_compute_total_stats_multiple_with_words() {
        let mut sf1 = StatsFile::new(Path::new("de.po"));
        sf1.entries = make_entries(10, 8, 1, 1, 0);
        sf1.words = Some(make_counts(50, 40, 5, 5, 0, 38, 4, 0, 0));
        sf1.chars = Some(make_counts(300, 240, 30, 30, 0, 230, 28, 0, 0));

        let mut sf2 = StatsFile::new(Path::new("fr.po"));
        sf2.entries = make_entries(20, 15, 3, 1, 1);
        sf2.words = Some(make_counts(100, 75, 15, 5, 5, 70, 12, 0, 4));
        sf2.chars = Some(make_counts(600, 450, 90, 30, 30, 420, 85, 0, 25));

        let total = compute_total_stats(&vec![sf1, sf2]);
        assert_eq!(total.entries.total, 30);
        assert_eq!(total.entries.translated, 23);
        assert_eq!(total.entries.fuzzy, 4);
        assert_eq!(total.entries.untranslated, 2);
        assert_eq!(total.entries.obsolete, 1);

        let words = total.words.unwrap();
        assert_eq!(words.id_total, 150);
        assert_eq!(words.id_translated, 115);
        assert_eq!(words.id_fuzzy, 20);
        assert_eq!(words.str_translated, 108);

        let chars = total.chars.unwrap();
        assert_eq!(chars.id_total, 900);
        assert_eq!(chars.id_translated, 690);
        assert_eq!(chars.str_translated, 650);

        assert!(total.path.display().to_string().contains("Total (2)"));
    }
}
