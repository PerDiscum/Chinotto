//! Real playback through the sound card (or a null device). Runs only when
//! CHINOTTO_TEST_PLAYBACK is set to a short music file (a few seconds long).

use std::sync::mpsc;
use std::time::{Duration, Instant};

use chinotto_player::{Event, Player, State};

#[test]
fn plays_pauses_seeks_and_ends() {
    let Some(file) = std::env::var_os("CHINOTTO_TEST_PLAYBACK") else { return };
    let (tx, rx) = mpsc::channel();
    let player = Player::new(move |e| {
        let _ = tx.send(e);
    });
    let wait = |pred: &dyn Fn(&Event) -> bool, secs: u64| -> Event {
        let end = Instant::now() + Duration::from_secs(secs);
        loop {
            let left = end.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left) {
                Ok(Event::Error(e)) => panic!("player error: {e}"),
                Ok(e) if pred(&e) => {
                    println!("{e:?}");
                    return e;
                }
                Ok(e) => println!("  {e:?}"),
                Err(_) => panic!("timed out"),
            }
        }
    };
    let is = |s: State| move |e: &Event| matches!(e, Event::Status(st) if st.state == s);

    player.load(file.into(), 0, true);
    wait(&is(State::Playing), 2);
    // Wait until sound is actually flowing (some outputs take a moment to wake up).
    wait(&|e| matches!(e, Event::Status(st) if st.position_ms >= 300), 5);
    player.pause();
    let Event::Status(p) = wait(&is(State::Paused), 2) else { unreachable!() };
    assert!((300..1200).contains(&p.position_ms), "paused at {} ms", p.position_ms);
    // Paused means paused: the position doesn't move.
    std::thread::sleep(Duration::from_millis(500));
    player.seek(1500);
    let Event::Status(s) = wait(&|e| matches!(e, Event::Status(st) if st.position_ms >= 1400), 2) else { unreachable!() };
    assert_eq!(s.state, State::Paused, "seeking keeps it paused");
    assert!(s.position_ms < 1600, "{}", s.position_ms);
    let resumed = Instant::now();
    player.play();
    wait(&|e| matches!(e, Event::Ended), 6);
    let took = resumed.elapsed();
    println!("ended {took:?} after resuming at 1.5 s of a 3.0 s file");
    // ~1.5 s left, plus output latency.
    assert!(took > Duration::from_millis(1200) && took < Duration::from_millis(2600), "{took:?}");
}
