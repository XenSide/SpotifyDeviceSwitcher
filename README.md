# Spotify Device Switcher

A native Windows utility written in Rust to switch Spotify playback to a target device via the Spotify Web API.

## Features
- **API Device Switching:** Easily transfer playback to any Spotify Connect device (AV receivers, smart speakers, TVs, etc.) using the Spotify Web API.
- **Dynamic Target Device:** You can change the target device on the fly using a command-line argument.
- **Silent Background Execution:** Runs without a console window by default so you can bind it to a hotkey or Stream Deck without annoying popups.
- **Debug Mode:** Run with the `-debug` flag to open a console window and view logs.
- **Token Caching:** Automatically caches your Spotify OAuth token in your local AppData folder (`%LOCALAPPDATA%\SpotifyDeviceSwitcher\token.json`) to keep your directory clean.
- **Environment Variables:** Configuration is easily handled via a `.env` file.

## Setup

1. **Create a Spotify Developer App**
   - Go to the [Spotify Developer Dashboard](https://developer.spotify.com/dashboard).
   - Create a new application.
   - Edit the settings and add a Redirect URI (e.g., `http://localhost:8888/callback`).
   - Note down your **Client ID** and **Client Secret**.

2. **Find your Target Device ID**
   - You can find your Device ID by using the Spotify Web API console or by logging it from a script while the device is active.

3. **Configure the Environment**
   - Create a `.env` file in the same directory as the executable with the following contents:
     ```env
     SPOTIFY_CLIENT_ID=your_client_id_here
     SPOTIFY_CLIENT_SECRET=your_client_secret_here
     SPOTIFY_REDIRECT_URI=http://localhost:8888/callback
     SPOTIFY_DEVICE_ID=your_default_target_device_id_here
     ```

4. **First Run**
   - Run the executable once manually (preferably with `-debug` so you can see what's happening).
   - It will open your web browser asking you to authorize the application.
   - After authorizing, you will be redirected to your Redirect URI. The script will automatically capture the code (or you may need to paste the URL back if prompted) and cache the token.
   - Future runs will happen entirely in the background.

## Usage
Simply run the executable. It is highly recommended to bind this to a macro key, a Stream Deck button, or a custom shortcut for quick access.

```cmd
:: Run silently and switch to the default device in your .env file
SpotifyDeviceSwitcher.exe

:: Switch to a specific device on the fly
SpotifyDeviceSwitcher.exe -device your_device_id_here

:: Run with a console window for debugging
SpotifyDeviceSwitcher.exe -debug

:: You can also combine arguments
SpotifyDeviceSwitcher.exe -debug -device your_device_id_here
```
