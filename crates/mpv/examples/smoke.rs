use patina_mpv::{Event, Mpv, SpawnOptions};
use serde_json::json;

#[tokio::main]
async fn main() {
    let (mpv, mut rx) = Mpv::spawn(SpawnOptions { extra_args: vec!["--vo=null".into(), "--ao=null".into()], ..Default::default() }).await.unwrap();
    println!("version: {}", mpv.get_property("mpv-version").await.unwrap());
    println!("config-dir: {:?}", mpv.get_property("config-dir").await);
    mpv.bind_message("Ctrl+k", "patina-skip").await.unwrap();
    mpv.command(vec![json!("script-message"), json!("patina-skip")]).await.unwrap();
    mpv.observe_property(1, "idle-active").await.unwrap();
    for _ in 0..4 {
        if let Ok(Some(ev)) = tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await {
            println!("event: {ev:?}");
            if matches!(ev, Event::ClientMessage { .. }) { break; }
        }
    }
    mpv.quit().await.unwrap();
    mpv.wait(std::time::Duration::from_secs(3)).await;
    println!("done");
}
