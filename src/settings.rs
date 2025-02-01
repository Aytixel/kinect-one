use std::time::Duration;

// The following information was found by using the library released by Microsoft under MIT license,
// https://github.com/Microsoft/MixedRealityCompanionKit/tree/master/KinectIPD/NuiSensor
#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum LedId {
    Primary = 0,
    Secondary = 1,
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum LedMode {
    Constant = 0,
    /// Blink between start level, stop level every interval
    Blink = 1,
}

// The following information was found by using the library released by Microsoft under MIT license,
// https://github.com/Microsoft/MixedRealityCompanionKit/tree/master/KinectIPD/NuiSensor
// Debugging the library assembly shows the original struct name was _PETRA_LED_STATE.
#[derive(Debug, Clone, Copy)]
pub struct LedSettings {
    id: LedId,
    mode: LedMode,
    /// LED intensity [0, 1000]
    start_level: u16,
    /// LED intensity [0, 1000]
    stop_level: u16,
    /// Blink interval
    interval: Duration,
}

impl LedSettings {
    /// Constant mode
    ///
    /// # Arguments
    ///
    /// * `id` - LED id
    /// * `level` - LED intensity [0, 1000]
    pub fn constant(id: LedId, level: u16) -> Self {
        Self {
            id,
            mode: LedMode::Constant,
            start_level: level,
            stop_level: 0,
            interval: Duration::from_secs(0),
        }
    }

    /// Blink mode
    ///
    /// # Arguments
    ///
    /// * `id` - LED id
    /// * `start_level` - LED intensity [0, 1000]
    /// * `stop_level` - LED intensity [0, 1000]
    /// * `interval` - Blink interval
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
        self.start_level.clamp(0, 1000)
    }

    pub fn stop_level(&self) -> u16 {
        self.stop_level.clamp(0, 1000)
    }

    pub fn interval(&self) -> Duration {
        self.interval
    }
}

#[derive(Debug, Clone, Copy)]
/// Parameters of depth processing.
pub struct DepthProcessorParams {
    pub ab_multiplier: f32,
    pub ab_multiplier_per_frq: [f32; 3],
    pub ab_output_multiplier: f32,

    pub phase_in_rad: [f32; 3],

    pub joint_bilateral_ab_threshold: f32,
    pub joint_bilateral_max_edge: f32,
    pub joint_bilateral_exp: f32,
    pub gaussian_kernel: [f32; 9],

    pub phase_offset: f32,
    pub unambiguous_dist: f32,
    pub individual_ab_threshold: f32,
    pub ab_threshold: f32,
    pub ab_confidence_slope: f32,
    pub ab_confidence_offset: f32,
    pub min_dealias_confidence: f32,
    pub max_dealias_confidence: f32,

    pub edge_ab_avg_min_value: f32,
    pub edge_ab_std_dev_threshold: f32,
    pub edge_close_delta_threshold: f32,
    pub edge_far_delta_threshold: f32,
    pub edge_max_delta_threshold: f32,
    pub edge_avg_delta_threshold: f32,
    pub max_edge_count: f32,

    pub kde_sigma_sqr: f32,
    pub unwrapping_likelihood_scale: f32,
    pub phase_confidence_scale: f32,
    pub kde_threshold: f32,
    pub kde_neigborhood_size: usize,
    pub num_hyps: usize,

    pub min_depth: f32,
    pub max_depth: f32,
}

impl Default for DepthProcessorParams {
    fn default() -> Self {
        Self {
            ab_multiplier: 0.6666667,
            ab_multiplier_per_frq: [1.322581, 1.0, 1.612903],
            ab_output_multiplier: 16.0,

            phase_in_rad: [0.0, 2.094395, 4.18879],

            joint_bilateral_ab_threshold: 3.0,
            joint_bilateral_max_edge: 2.5,
            joint_bilateral_exp: 5.0,

            gaussian_kernel: [
                0.1069973, 0.1131098, 0.1069973, 0.1131098, 0.1195716, 0.1131098, 0.1069973,
                0.1131098, 0.1069973,
            ],

            phase_offset: 0.0,
            unambiguous_dist: 2083.333,
            individual_ab_threshold: 3.0,
            ab_threshold: 10.0,
            ab_confidence_slope: -0.5330578,
            ab_confidence_offset: 0.7694894,
            min_dealias_confidence: 0.3490659,
            max_dealias_confidence: 0.6108653,

            edge_ab_avg_min_value: 50.0,
            edge_ab_std_dev_threshold: 0.05,
            edge_close_delta_threshold: 50.0,
            edge_far_delta_threshold: 30.0,
            edge_max_delta_threshold: 100.0,
            edge_avg_delta_threshold: 0.0,
            max_edge_count: 5.0,

            /*
             * These are parameters for the method described in "Efficient Phase Unwrapping
             * using Kernel Density Estimation", ECCV 2016, Felix Järemo Lawin, Per-Erik Forssen and
             * Hannes Ovren, see http://www.cvl.isy.liu.se/research/datasets/kinect2-dataset/.
             */
            kde_sigma_sqr: 0.0239282226563, //the scale of the kernel in the KDE, h in eq (13).
            unwrapping_likelihood_scale: 2.0, //scale parameter for the unwrapping likelihood, s_1^2 in eq (15).
            phase_confidence_scale: 3.0, //scale parameter for the phase likelihood, s_2^2 in eq (23)
            kde_threshold: 0.5, //threshold on the KDE output in eq (25), defines the inlier/outlier rate trade-off

            kde_neigborhood_size: 5, //spatial support of the KDE, defines a filter size of (2*kde_neigborhood_size+1 x 2*kde_neigborhood_size+1)
            num_hyps: 2, //number of phase unwrapping hypothesis considered by the KDE in each pixel. Implemented values are 2 and 3.
            //a large kde_neigborhood_size improves performance but may remove fine structures and makes the processing slower.
            //setting num_hyp to 3 improves the performance slightly but makes processing slower
            min_depth: 500.0,
            max_depth: 4500.0, //set to > 8000 for best performance when using the kde pipeline
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PacketParams {
    pub max_iso_packet_size: u16,
    pub rgb_transfer_size: usize,
    pub rgb_num_transfers: usize,
    pub ir_packets_per_transfer: i32,
    pub ir_num_transfers: usize,
}

impl Default for PacketParams {
    fn default() -> Self {
        if cfg!(target_os = "macos") {
            Self {
                max_iso_packet_size: 0,
                rgb_transfer_size: 0x4000,
                rgb_num_transfers: 20,
                ir_packets_per_transfer: 128,
                ir_num_transfers: 4,
            }
        } else if cfg!(target_os = "windows") {
            Self {
                max_iso_packet_size: 0,
                rgb_transfer_size: 1048576,
                rgb_num_transfers: 3,
                ir_packets_per_transfer: 64,
                ir_num_transfers: 8,
            }
        } else {
            Self {
                max_iso_packet_size: 0,
                rgb_transfer_size: 0x4000,
                rgb_num_transfers: 20,
                ir_packets_per_transfer: 8,
                ir_num_transfers: 60,
            }
        }
    }
}
