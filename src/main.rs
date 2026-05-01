#![windows_subsystem = "windows"]

use rspotify::{prelude::*, scopes, AuthCodeSpotify, Config, Credentials, OAuth};
use std::time::Duration;
use tokio::time::sleep;

async fn run_wake_pause(
    spotify: &AuthCodeSpotify,
    device_id: &str,
    target_volume: u8,
) -> Result<(), Box<dyn std::error::Error>> {
    let current_playback = spotify.current_playback(None, None::<Vec<_>>).await?;

    // Check if we are already playing on the target device
    if let Some(ref playback) = current_playback {
        if playback.device.id.as_deref() == Some(device_id) {
            return Ok(());
        }
    }

    let is_playing = current_playback.as_ref().map_or(false, |p| p.is_playing);

    if is_playing {
        spotify.transfer_playback(device_id, Some(true)).await?;
        println!("Music was playing. Transferred and kept playing!");
    } else {
        // Try a silent transfer first (works if the device is already awake)
        spotify.transfer_playback(device_id, Some(false)).await?;

        // Give the API a moment to process, then check if the device actually woke up
        sleep(Duration::from_millis(300)).await;
        let check = spotify.current_playback(None, None::<Vec<_>>).await?;
        let device_active = check
            .as_ref()
            .and_then(|p| p.device.id.as_deref())
            == Some(device_id);

        if device_active {
            println!("Music was paused. Transferred to device (paused state preserved).");
        } else {
            // Device is cold/sleeping — force-wake it by playing at volume 0
            println!("Device didn't wake from silent transfer. Force-waking at zero volume...");

            // 1. Drop Spotify volume to 0 via API before any play command
            // Explicitly target the device_id to avoid 403 errors if the current active device is a phone
            spotify.volume(0, Some(device_id)).await?;

            // 2. Force the device to wake up by starting playback (at volume 0)
            spotify.transfer_playback(device_id, Some(true)).await?;

            // 3. Give Spotify servers a moment to register the device
            sleep(Duration::from_millis(300)).await;

            // 4. Pause playback
            spotify.pause_playback(Some(device_id)).await?;

            // 5. Restore original volume
            spotify.volume(target_volume, Some(device_id)).await?;
            println!("Device awakened and re-paused. Volume restored to {}%.", target_volume);
        }
    }

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(e) = run_app().await {
        let msg: Vec<u16> = format!("Error: {}\0", e).encode_utf16().collect();
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::MessageBoxW(
                None,
                windows::core::PCWSTR(msg.as_ptr()),
                windows::core::w!("SpotifyDeviceSwitcher Error"),
                windows::Win32::UI::WindowsAndMessaging::MB_ICONERROR | windows::Win32::UI::WindowsAndMessaging::MB_OK
            );
        }
    }
}

async fn run_app() -> Result<(), String> {
    // If -debug is passed, allocate a console window to show output
    let is_debug = std::env::args().any(|arg| arg == "-debug");
    if is_debug {
        unsafe {
            let _ = windows::Win32::System::Console::AllocConsole();
        }
    }

    // Load environment variables from .env file next to the executable
    let mut env_loaded = false;
    if let Ok(mut exe_path) = std::env::current_exe() {
        exe_path.pop();
        exe_path.push(".env");
        if dotenvy::from_path(&exe_path).is_ok() {
            env_loaded = true;
        }
    }
    
    // Fallback: search in current working directory and upwards
    if !env_loaded {
        let _ = dotenvy::dotenv();
    }

    let mut override_device_id = None;
    let args: Vec<String> = std::env::args().collect();
    for i in 1..args.len() {
        if args[i] == "-device" && i + 1 < args.len() {
            override_device_id = Some(args[i + 1].clone());
        }
    }

    let client_id = std::env::var("SPOTIFY_CLIENT_ID").map_err(|_| "SPOTIFY_CLIENT_ID must be set in env or .env".to_string())?;
    let client_secret = std::env::var("SPOTIFY_CLIENT_SECRET").map_err(|_| "SPOTIFY_CLIENT_SECRET must be set in env or .env".to_string())?;
    let redirect_uri = std::env::var("SPOTIFY_REDIRECT_URI").map_err(|_| "SPOTIFY_REDIRECT_URI must be set in env or .env".to_string())?;
    let device_id = override_device_id
        .or_else(|| std::env::var("SPOTIFY_DEVICE_ID").ok())
        .ok_or_else(|| "Device ID must be provided via -device argument or SPOTIFY_DEVICE_ID env var".to_string())?;

    let creds = Credentials::new(&client_id, &client_secret);
    let oauth = OAuth {
        redirect_uri,
        scopes: scopes!("user-modify-playback-state", "user-read-playback-state"),
        ..Default::default()
    };

    // Configure caching to use AppData to keep the working directory clean
    let mut config = Config::default();
    if let Some(mut data_dir) = dirs::data_local_dir() {
        data_dir.push("SpotifyDeviceSwitcher");
        std::fs::create_dir_all(&data_dir).ok();
        data_dir.push("token.json");
        config.token_cached = true;
        config.cache_path = data_dir;
    }

    let spotify = AuthCodeSpotify::with_config(creds, oauth, config);
    let url = spotify.get_authorize_url(false).map_err(|e| e.to_string())?;
    
    // If the token isn't valid, it will try to prompt. This fails without a console.
    match spotify.prompt_for_token(&url).await {
        Ok(_) => {},
        Err(e) => return Err(format!("Failed to authenticate. If this is your first time, please run with -debug from the command prompt. Error: {}", e)),
    }

    // Fetch the target device's explicit volume before we start, so we can restore it exactly
    let mut target_volume: u8 = 50; // Fallback
    if let Ok(devices) = spotify.device().await {
        for d in devices {
            if d.id.as_deref() == Some(device_id.as_str()) {
                if let Some(vol) = d.volume_percent {
                    target_volume = vol as u8;
                }
                break;
            }
        }
    }

    // Retry up to 10 times on 404 errors — the Spotify app may have just opened
    // and the API needs a moment to register the device
    let max_retries = 10;
    let mut last_err = String::new();
    for attempt in 1..=max_retries {
        match run_wake_pause(&spotify, &device_id, target_volume).await {
            Ok(_) => {
                last_err.clear();
                break;
            }
            Err(e) => {
                let err_str = e.to_string();
                let is_404 = err_str.contains("404");
                if is_404 && attempt < max_retries {
                    println!("Attempt {}/{}: Device not found (404), retrying in 500ms...", attempt, max_retries);
                    sleep(Duration::from_millis(500)).await;
                    last_err = err_str;
                    continue;
                }
                // Not a 404 or final attempt — bail out
                // Failsafe: restore volume in case we crashed mid-mute
                let _ = spotify.volume(target_volume, Some(&device_id)).await;
                return Err(format!("An error occurred: {}", e));
            }
        }
    }
    if !last_err.is_empty() {
        // Failsafe: restore volume in case we crashed mid-mute
        let _ = spotify.volume(target_volume, Some(&device_id)).await;
        return Err(format!("An error occurred after {} retries: {}", max_retries, last_err));
    }

    Ok(())
}
