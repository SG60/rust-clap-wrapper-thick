use nih_plug::prelude::*;
use parking_lot::Mutex;
use std::sync::Arc;

mod editor;

pub struct RustPlugin1 {
    params: Arc<GainParams>,

    /// Needed to normalize the peak meter's response based on the sample rate.
    peak_meter_decay_weight: f32,
    /// The current data for the peak meter. This is stored as an [`Arc`] so we can share it between
    /// the GUI and the audio processing parts. If you have more state to share, then it's a good
    /// idea to put all of that in a struct behind a single `Arc`.
    ///
    /// This is stored as voltage gain.
    peak_meter: Arc<AtomicF32>,
}

/// The [`Params`] derive macro gathers all of the information needed for the wrapper to know about
/// the plugin's parameters, persistent serializable fields, and nested parameter groups. You can
/// also easily implement [`Params`] by hand if you want to, for instance, have multiple instances
/// of a parameters struct for multiple identical oscillators/filters/envelopes.
#[derive(Params)]
struct GainParams {
    /// The parameter's ID is used to identify the parameter in the wrapped plugin API. As long as
    /// these IDs remain constant, you can rename and reorder these fields as you wish. The
    /// parameters are exposed to the host in the same order they were defined. In this case, this
    /// gain parameter is stored as linear gain while the values are displayed in decibels.
    #[id = "gain"]
    pub gain: FloatParam,

    /// This field isn't used in this example, but anything written to the vector would be restored
    /// together with a preset/state file saved for this plugin. This can be useful for storing
    /// things like sample data.
    #[persist = "industry_secrets"]
    pub random_data: Mutex<Vec<f32>>,

    /// You can also nest parameter structs. These will appear as a separate nested group if your
    /// DAW displays parameters in a tree structure.
    #[nested(group = "Subparameters")]
    pub sub_params: SubParams,

    /// Nested parameters also support some advanced functionality for reusing the same parameter
    /// struct multiple times.
    #[nested(array, group = "Array Parameters")]
    pub array_params: [ArrayParams; 3],

    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<nih_plug_iced::IcedState>,
}

#[derive(Params)]
struct SubParams {
    #[id = "thing"]
    pub nested_parameter: FloatParam,
}

#[derive(Params)]
struct ArrayParams {
    /// This parameter's ID will get a `_1`, `_2`, and a `_3` suffix because of how it's used in
    /// `array_params` above.
    #[id = "noope"]
    pub nope: FloatParam,
}

impl Default for RustPlugin1 {
    fn default() -> Self {
        Self {
            params: Arc::new(GainParams::default()),

            peak_meter_decay_weight: 1.0,
            peak_meter: Arc::new(AtomicF32::new(util::MINUS_INFINITY_DB)),
        }
    }
}

impl Default for GainParams {
    fn default() -> Self {
        Self {
            editor_state: editor::default_state(),

            // This gain is stored as linear gain. NIH-plug comes with useful conversion functions
            // to treat these kinds of parameters as if we were dealing with decibels. Storing this
            // as decibels is easier to work with, but requires a conversion for every sample.
            gain: FloatParam::new(
                "Gain",
                util::db_to_gain(0.0),
                FloatRange::Skewed {
                    min: util::db_to_gain(-30.0),
                    max: util::db_to_gain(30.0),
                    // This makes the range appear as if it was linear when displaying the values as
                    // decibels
                    factor: FloatRange::gain_skew_factor(-30.0, 30.0),
                },
            )
            // Because the gain parameter is stored as linear gain instead of storing the value as
            // decibels, we need logarithmic smoothing
            .with_smoother(SmoothingStyle::Logarithmic(50.0))
            .with_unit(" dB")
            // There are many predefined formatters we can use here. If the gain was stored as
            // decibels instead of as a linear gain value, we could have also used the
            // `.with_step_size(0.1)` function to get internal rounding.
            .with_value_to_string(formatters::v2s_f32_gain_to_db(2))
            .with_string_to_value(formatters::s2v_f32_gain_to_db()),
            // Persisted fields can be initialized like any other fields, and they'll keep their
            // values when restoring the plugin's state.
            random_data: Mutex::new(Vec::new()),
            sub_params: SubParams {
                nested_parameter: FloatParam::new(
                    "Unused Nested Parameter",
                    0.5,
                    FloatRange::Skewed {
                        min: 2.0,
                        max: 2.4,
                        factor: FloatRange::skew_factor(2.0),
                    },
                )
                .with_value_to_string(formatters::v2s_f32_rounded(2)),
            },
            array_params: [1, 2, 3].map(|index| ArrayParams {
                nope: FloatParam::new(
                    format!("Nope {index}"),
                    0.5,
                    FloatRange::Linear { min: 1.0, max: 2.0 },
                ),
            }),
        }
    }
}

impl Plugin for RustPlugin1 {
    const NAME: &'static str = "Gain";
    const VENDOR: &'static str = "A Fantastic Company";
    // You can use `env!("CARGO_PKG_HOMEPAGE")` to reference the homepage field from the
    // `Cargo.toml` file here
    const URL: &'static str = "";
    const EMAIL: &'static str = "info@example.com";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // The first audio IO layout is used as the default. The other layouts may be selected either
    // explicitly or automatically by the host or the user depending on the plugin API/backend.
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),

            aux_input_ports: &[],
            aux_output_ports: &[],

            // Individual ports and the layout as a whole can be named here. By default these names
            // are generated as needed. This layout will be called 'Stereo', while the other one is
            // given the name 'Mono' based no the number of input and output channels.
            names: PortNames::const_default(),
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    // Setting this to `true` will tell the wrapper to split the buffer up into smaller blocks
    // whenever there are inter-buffer parameter changes. This way no changes to the plugin are
    // required to support sample accurate automation and the wrapper handles all of the boring
    // stuff like making sure transport and other timing information stays consistent between the
    // splits.
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    // If the plugin can send or receive SysEx messages, it can define a type to wrap around those
    // messages here. The type implements the `SysExMessage` trait, which allows conversion to and
    // from plain byte buffers.
    type SysExMessage = ();
    // More advanced plugins can use this to run expensive background tasks. See the field's
    // documentation for more information. `()` means that the plugin does not have any background
    // tasks.
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        editor::create(
            self.params.clone(),
            self.peak_meter.clone(),
            self.params.editor_state.clone(),
        )
    }

    // This plugin doesn't need any special initialization, but if you need to do anything expensive
    // then this would be the place. State is kept around when the host reconfigures the
    // plugin. If we do need special initialization, we could implement the `initialize()` and/or
    // `reset()` methods

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        for channel_samples in buffer.iter_samples() {
            let mut amplitude = 0.0;
            let num_samples = channel_samples.len();

            // Smoothing is optionally built into the parameters themselves
            let gain = self.params.gain.smoothed.next();

            for sample in channel_samples {
                *sample *= gain;
                amplitude += *sample;
            }

            // To save resources, a plugin can (and probably should!) only perform expensive
            // calculations that are only displayed on the GUI while the GUI is open
            if self.params.editor_state.is_open() {
                amplitude = (amplitude / num_samples as f32).abs();
                let current_peak_meter = self.peak_meter.load(std::sync::atomic::Ordering::Relaxed);
                let new_peak_meter = if amplitude > current_peak_meter {
                    amplitude
                } else {
                    current_peak_meter * self.peak_meter_decay_weight
                        + amplitude * (1.0 - self.peak_meter_decay_weight)
                };

                self.peak_meter
                    .store(new_peak_meter, std::sync::atomic::Ordering::Relaxed)
            }
        }

        ProcessStatus::Normal
    }

    // This can be used for cleaning up special resources like socket connections whenever the
    // plugin is deactivated. Most plugins won't need to do anything here.
    fn deactivate(&mut self) {}
}

impl ClapPlugin for RustPlugin1 {
    const CLAP_ID: &'static str = "org.free-audio.clap-rust-gain";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A smoothed gain parameter example plugin");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Utility,
    ];
}

#[doc(hidden)]
pub mod clap {
    use super::*;
    use ::std::collections::HashSet;
    use ::std::ffi::{c_void, CStr};
    use ::std::os::raw::c_char;
    use ::std::sync::{Arc, OnceLock};
    use nih_plug::prelude::nih_debug_assert_eq;
    use nih_plug::wrapper::clap::{
        clap_host, clap_plugin, clap_plugin_descriptor, clap_plugin_factory, CLAP_PLUGIN_FACTORY_ID,
    };
    use nih_plug::wrapper::clap::{PluginDescriptor, Wrapper};
    use nih_plug::wrapper::setup_logger;
    const CLAP_PLUGIN_FACTORY: clap_plugin_factory = clap_plugin_factory {
        get_plugin_count: Some(get_plugin_count),
        get_plugin_descriptor: Some(get_plugin_descriptor),
        create_plugin: Some(create_plugin),
    };
    const PLUGIN_COUNT: usize = [stringify!(RustPlugin1)].len();
    static PLUGIN_DESCRIPTORS: OnceLock<[PluginDescriptor; PLUGIN_COUNT]> = OnceLock::new();
    fn plugin_descriptors() -> &'static [PluginDescriptor; PLUGIN_COUNT] {
        PLUGIN_DESCRIPTORS.get_or_init(|| {
            let descriptors = [PluginDescriptor::for_plugin::<RustPlugin1>()];
            if cfg!(debug_assertions) {
                let unique_plugin_ids: HashSet<_> =
                    descriptors.iter().map(|d| d.clap_id()).collect();
                nih_debug_assert_eq!(
                    unique_plugin_ids.len(),
                    descriptors.len(),
                    "Duplicate plugin IDs found in `nih_export_clap!()` call"
                );
            }
            descriptors
        })
    }
    unsafe extern "C" fn get_plugin_count(_factory: *const clap_plugin_factory) -> u32 {
        plugin_descriptors().len() as u32
    }
    unsafe extern "C" fn get_plugin_descriptor(
        _factory: *const clap_plugin_factory,
        index: u32,
    ) -> *const clap_plugin_descriptor {
        match plugin_descriptors().get(index as usize) {
            Some(descriptor) => descriptor.clap_plugin_descriptor(),
            None => ::std::ptr::null(),
        }
    }
    unsafe extern "C" fn create_plugin(
        _factory: *const clap_plugin_factory,
        host: *const clap_host,
        plugin_id: *const c_char,
    ) -> *const clap_plugin {
        if plugin_id.is_null() {
            return ::std::ptr::null();
        }
        let plugin_id_cstr = CStr::from_ptr(plugin_id);
        let descriptors = plugin_descriptors();
        let mut descriptor_idx = 0;
        {
            let descriptor = &descriptors[descriptor_idx];
            if plugin_id_cstr == descriptor.clap_id() {
                return (*Arc::into_raw(Wrapper::<RustPlugin1>::new(host)))
                    .clap_plugin
                    .as_ptr();
            }
            descriptor_idx += 1;
        }
        ::std::ptr::null()
    }
    pub extern "C" fn init(_plugin_path: *const c_char) -> bool {
        setup_logger();
        true
    }
    pub extern "C" fn deinit() {}

    pub extern "C" fn get_factory(factory_id: *const c_char) -> *const c_void {
        if !factory_id.is_null() && unsafe { CStr::from_ptr(factory_id) } == CLAP_PLUGIN_FACTORY_ID
        {
            &CLAP_PLUGIN_FACTORY as *const _ as *const c_void
        } else {
            ::std::ptr::null()
        }
    }
}

#[doc = r" The CLAP plugin's entry point."]
#[no_mangle]
#[used]
pub static rust_clap_entry: nih_plug::wrapper::clap::clap_plugin_entry =
    nih_plug::wrapper::clap::clap_plugin_entry {
        clap_version: nih_plug::wrapper::clap::CLAP_VERSION,
        init: Some(self::clap::init),
        deinit: Some(self::clap::deinit),
        get_factory: Some(self::clap::get_factory),
    };

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
