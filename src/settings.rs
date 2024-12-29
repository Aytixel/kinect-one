use std::time::Duration;

// The following information was found by using the library released by Microsoft under MIT license,
// https://github.com/Microsoft/MixedRealityCompanionKit/tree/master/KinectIPD/NuiSensor
#[derive(Clone, Copy)]
#[repr(u32)]
pub enum ColorSettingCommandType {
    SetExposureMode = 0,
    SetIntegrationTime = 1,
    GetIntegrationTime = 2,
    SetWhiteBalanceMode = 10,
    SetReChannelGain = 11,
    SetGreenChannelGain = 12,
    SetBlueChannelGain = 13,
    GetRedChannelGain = 14,
    GetGreenChannelGain = 15,
    GetBlueChannelGain = 16,
    SetExposureTimeMs = 17,
    GetExposureTimeMs = 18,
    SetDigitalGain = 19,
    GetDigitalGain = 20,
    SetAnalogGain = 21,
    GetAnalogGain = 22,
    SetExposureCompensation = 23,
    GetExposureCompensation = 24,
    SetAcs = 25,
    GetAcs = 26,
    SetExposureMeteringMode = 27,
    SetExposureMeteringZones = 28,
    SetExposureMeteringZone0Weight = 29,
    SetExposureMeteringZone1Weight = 30,
    SetExposureMeteringZone2Weight = 31,
    SetExposureMeteringZone3Weight = 32,
    SetExposureMeteringZone4Weight = 33,
    SetExposureMeteringZone5Weight = 34,
    SetExposureMeteringZone6Weight = 35,
    SetExposureMeteringZone7Weight = 36,
    SetExposureMeteringZone8Weight = 37,
    SetExposureMeteringZone9Weight = 38,
    SetExposureMeteringZone10Weight = 39,
    SetExposureMeteringZone11Weight = 40,
    SetExposureMeteringZone12Weight = 41,
    SetExposureMeteringZone13Weight = 42,
    SetExposureMeteringZone14Weight = 43,
    SetExposureMeteringZone15Weight = 44,
    SetExposureMeteringZone16Weight = 45,
    SetExposureMeteringZone17Weight = 46,
    SetExposureMeteringZone18Weight = 47,
    SetExposureMeteringZone19Weight = 48,
    SetExposureMeteringZone20Weight = 49,
    SetExposureMeteringZone21Weight = 50,
    SetExposureMeteringZone22Weight = 51,
    SetExposureMeteringZone23Weight = 52,
    SetExposureMeteringZone24Weight = 53,
    SetExposureMeteringZone25Weight = 54,
    SetExposureMeteringZone26Weight = 55,
    SetExposureMeteringZone27Weight = 56,
    SetExposureMeteringZone28Weight = 57,
    SetExposureMeteringZone29Weight = 58,
    SetExposureMeteringZone30Weight = 59,
    SetExposureMeteringZone31Weight = 60,
    SetExposureMeteringZone32Weight = 61,
    SetExposureMeteringZone33Weight = 62,
    SetExposureMeteringZone34Weight = 63,
    SetExposureMeteringZone35Weight = 64,
    SetExposureMeteringZone36Weight = 65,
    SetExposureMeteringZone37Weight = 66,
    SetExposureMeteringZone38Weight = 67,
    SetExposureMeteringZone39Weight = 68,
    SetExposureMeteringZone40Weight = 69,
    SetExposureMeteringZone41Weight = 70,
    SetExposureMeteringZone42Weight = 71,
    SetExposureMeteringZone43Weight = 72,
    SetExposureMeteringZone44Weight = 73,
    SetExposureMeteringZone45Weight = 74,
    SetExposureMeteringZone46Weight = 75,
    SetExposureMeteringZone47Weight = 76,
    SetMaxAnalogGainCap = 77,
    SetMaxDigitalGainCap = 78,
    SetFlickerFreeFrequency = 79,
    GetExposureMode = 80,
    GetWhiteBalanceMode = 81,
    SetFrameRate = 82,
    GetFrameRate = 83,
}

#[derive(Clone, Copy)]
#[repr(u16)]
pub enum LedId {
    Primary = 0,
    Secondary = 1,
}

#[derive(Clone, Copy)]
#[repr(u16)]
pub enum LedMode {
    Constant = 0,
    /// Blink between start level, stop level every interval
    Blink = 1,
}

// The following information was found by using the library released by Microsoft under MIT license,
// https://github.com/Microsoft/MixedRealityCompanionKit/tree/master/KinectIPD/NuiSensor
// Debugging the library assembly shows the original struct name was _PETRA_LED_STATE.
#[derive(Clone, Copy)]
pub struct LedSettings {
    id: LedId,
    mode: LedMode,
    /// LED intensity  [0, 1000]
    start_level: u16,
    /// LED intensity  [0, 1000]
    stop_level: u16,
    /// Blink interval
    interval: Duration,
}

impl LedSettings {
    pub fn constant(id: LedId, level: u16) -> Self {
        Self {
            id,
            mode: LedMode::Constant,
            start_level: level,
            stop_level: 0,
            interval: Duration::from_secs(0),
        }
    }

    pub fn blink(id: LedId, start_level: u16, stop_level: u16, interval: Duration) -> Self {
        Self {
            id,
            mode: LedMode::Blink,
            start_level,
            stop_level,
            interval,
        }
    }

    pub fn id(&self) -> LedId {
        self.id
    }

    pub fn mode(&self) -> LedMode {
        self.mode
    }

    pub fn start_level(&self) -> u16 {
        if self.start_level > 1000 {
            1000
        } else {
            self.start_level
        }
    }

    pub fn stop_level(&self) -> u16 {
        if self.start_level > 1000 {
            1000
        } else {
            self.stop_level
        }
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }
}
