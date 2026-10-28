use std::path::PathBuf;

use fix_whxm4_voice::changes::Change;
use fix_whxm4_voice::size::{check, limit};
use fix_whxm4_voice::slots::{SLOT_COUNT, clip_file_name};
use fix_whxm4_voice::workspace::Workspace;

/// Stock clips of 1920 bytes each, except slot 1, of 1440 bytes.
fn stock() -> Vec<Vec<u8>> {
    (0..SLOT_COUNT)
        .map(|slot| vec![0; if slot == 1 { 1440 } else { 1920 }])
        .collect()
}

fn ws() -> Workspace {
    Workspace {
        original: PathBuf::from("original"),
        changes: PathBuf::from("changes"),
        build: PathBuf::from("build"),
        stock: PathBuf::from("stock"),
    }
}

fn replace(name: &str) -> Change {
    Change::Replace(name.to_string())
}

/// `name` in the workspace folder `dir`, as the report writes it: with the OS's path separator.
fn in_dir(dir: &str, name: &str) -> String {
    PathBuf::from(dir).join(name).display().to_string()
}

/// Every slot keeps its original, except for `changes`, given as (slot, change, converted size).
fn built(changes: &[(usize, Change, usize)]) -> (Vec<Change>, Vec<Vec<u8>>) {
    let mut resolved = vec![Change::Original; SLOT_COUNT];
    let mut clips = stock();
    for (slot, change, size) in changes {
        resolved[*slot] = change.clone();
        clips[*slot] = vec![0; *size];
    }
    (resolved, clips)
}

fn report(changes: &[(usize, Change, usize)]) -> String {
    let (resolved, clips) = built(changes);
    check(&clips, &resolved, &limit(&stock()), &ws()).expect_err("the size check passed")
}

#[test]
fn the_limit_is_the_stock_clip_total() {
    assert_eq!(limit(&stock()).bytes, 53 * 1920 + 1440);
}

#[test]
fn a_pack_within_the_limit_passes() {
    let (resolved, clips) = built(&[
        (0, replace("00-long.wav"), 1920 + 500),
        (2, replace("02-short.wav"), 1920 - 500),
        (3, Change::Silence, 100),
    ]);

    check(&clips, &resolved, &limit(&stock()), &ws()).unwrap();
}

#[test]
fn the_report_lists_every_clip_the_total_and_the_limit() {
    let report = report(&[
        (0, replace("00-song.wav"), 16_000),
        (2, Change::Silence, 100),
    ]);

    for slot in 3..SLOT_COUNT {
        assert!(
            report.contains(&format!(
                "1920  {}",
                in_dir("original", &clip_file_name(slot))
            )),
            "{report}"
        );
    }
    assert!(
        report.contains(&format!("16000  {}", in_dir("changes", "00-song.wav"))),
        "{report}"
    );
    assert!(report.contains("100  silence"), "{report}");
    assert!(
        report.contains(&format!("1440  {}", in_dir("original", "01-power-off.mp3"))),
        "{report}"
    );
    let total = 16_000 + 100 + 1440 + 51 * 1920;
    assert!(report.contains(&format!("total {total}")), "{report}");
    assert!(
        report.contains(&format!("limit {}", 53 * 1920 + 1440)),
        "{report}"
    );
    assert!(
        report.contains("stock clips of the base language"),
        "{report}"
    );
}

#[test]
fn changed_clips_are_marked() {
    let report = report(&[(0, replace("00-song.wav"), 16_000)]);

    let line = |name: &str| {
        report
            .lines()
            .find(|line| line.contains(name))
            .unwrap()
            .to_string()
    };
    assert!(
        line(&in_dir("changes", "00-song.wav")).contains('*'),
        "{report}"
    );
    assert!(
        !line(&in_dir("original", "02-bluetooth-pairing.mp3")).contains('*'),
        "{report}"
    );
}

#[test]
fn the_report_asks_for_trimming_and_says_the_clips_are_already_converted() {
    let report = report(&[(0, replace("00-song.wav"), 16_000)]);

    assert!(report.contains("14080 more than the limit"), "{report}");
    assert!(
        report.contains("already at the bitrate of its slot"),
        "{report}"
    );
    assert!(
        report.contains("Trim changes in changes, or copy originals from original into changes and trim them there"),
        "{report}"
    );
    assert!(!report.contains("ffmpeg"), "{report}");
}
