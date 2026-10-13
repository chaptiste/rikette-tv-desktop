//! WASAPI: the audio devices Windows knows about, and the default output's volume.

use capture_protocol::{AudioDeviceInfo, AudioDevices};
use windows::Win32::Devices::FunctionDiscovery::PKEY_Device_FriendlyName;
use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, EDataFlow, IMMDeviceEnumerator, MMDeviceEnumerator, DEVICE_STATE_ACTIVE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED, STGM_READ,
};
use windows::Win32::System::Com::StructuredStorage::PropVariantToStringAlloc;

/// Once per process, on the thread that makes the calls.
pub fn init_com() {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

fn enumerator() -> windows::core::Result<IMMDeviceEnumerator> {
    unsafe { CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) }
}

fn list(enumerator: &IMMDeviceEnumerator, flow: EDataFlow) -> windows::core::Result<Vec<AudioDeviceInfo>> {
    let mut devices = Vec::new();
    unsafe {
        let collection = enumerator.EnumAudioEndpoints(flow, DEVICE_STATE_ACTIVE)?;
        for index in 0..collection.GetCount()? {
            let device = collection.Item(index)?;
            let id_ptr = device.GetId()?;
            let id = id_ptr.to_string().unwrap_or_default();
            CoTaskMemFree(Some(id_ptr.0 as *const _));
            let label = device
                .OpenPropertyStore(STGM_READ)
                .and_then(|store| store.GetValue(&PKEY_Device_FriendlyName))
                .and_then(|value| {
                    let text = PropVariantToStringAlloc(&value)?;
                    let label = text.to_string().unwrap_or_default();
                    CoTaskMemFree(Some(text.0 as *const _));
                    Ok(label)
                })
                .unwrap_or_default();
            if !id.is_empty() {
                let label = label.replace(['\t', '\r', '\n'], " ").trim().to_string();
                devices.push(AudioDeviceInfo { label: if label.is_empty() { id.clone() } else { label }, id });
            }
        }
    }
    Ok(devices)
}

pub fn devices() -> AudioDevices {
    let Ok(enumerator) = enumerator() else { return AudioDevices::default() };
    AudioDevices {
        outputs: list(&enumerator, eRender).unwrap_or_default(),
        inputs: list(&enumerator, eCapture).unwrap_or_default(),
    }
}

/// The default output's volume, 0–1, and 0 when muted. None when there is no output.
pub fn output_volume() -> Option<f32> {
    unsafe {
        let device = enumerator().ok()?.GetDefaultAudioEndpoint(eRender, eConsole).ok()?;
        let volume: IAudioEndpointVolume = device.Activate(CLSCTX_ALL, None).ok()?;
        if volume.GetMute().ok()?.as_bool() {
            return Some(0.0);
        }
        volume.GetMasterVolumeLevelScalar().ok()
    }
}
