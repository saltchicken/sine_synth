use nih_plug::prelude::*;
use fundsp::hacker32::*;
use std::sync::Arc;

struct SineSynth {
    params: Arc<SineSynthParams>,
    
    // The compiled fundsp audio graph.
    graph: Box<dyn AudioUnit>,
    
    // Shared variables allow us to trigger pitch and amplitude from MIDI notes
    freq_var: Shared,
    amp_var: Shared,
}

#[derive(Params)]
struct SineSynthParams {
    #[id = "gain"]
    pub gain: FloatParam,
}

impl Default for SineSynthParams {
    fn default() -> Self {
        Self {
            gain: FloatParam::new(
                "Gain",
                0.5,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_unit(" %")
            .with_value_to_string(formatters::v2s_f32_percentage(2)),
        }
    }
}

impl Default for SineSynth {
    fn default() -> Self {
        // 1. Create our shared variables to communicate with the graph
        let freq_var = shared(440.0);
        let amp_var = shared(0.0);
        
        // 2. Build the basic Sine oscillator
        // `var(&freq_var) >> sine()` turns our frequency variable into a sine wave.
        // `* var(&amp_var)` acts as our basic volume envelope triggered by MIDI velocity.
        let synth = (var(&freq_var) >> sine()) * var(&amp_var);
        
        // 3. Duplicate our mono synthesizer into a stereo field
        let stereo_synth = synth.clone() | synth;

        Self {
            params: Arc::new(SineSynthParams::default()),
            graph: Box::new(stereo_synth),
            freq_var,
            amp_var,
        }
    }
}

impl Plugin for SineSynth {
    const NAME: &'static str = "Sine Synth";
    const VENDOR: &'static str = "Your Name";
    const URL: &'static str = env!("CARGO_PKG_HOMEPAGE");
    const EMAIL: &'static str = "your@email.com";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: None, // It's a synth, so it doesn't take audio input!
        main_output_channels: NonZeroU32::new(2),
        aux_input_ports: &[],
        aux_output_ports: &[],
        names: PortNames::const_default(),
    }];

    // Enable basic MIDI input for NoteOn/NoteOff
    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = true;
    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.graph.reset();
        self.graph.set_sample_rate(buffer_config.sample_rate as f64);
        true
    }

    fn reset(&mut self) {
        self.graph.reset();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        
        // Grab the next incoming MIDI event
        let mut next_event = context.next_event();

        // Iterate through the DAW audio buffer sample by sample
        for (sample_id, mut frame) in buffer.iter_samples().enumerate() {
            
            // Process all MIDI events that occur precisely on this sample
            while let Some(event) = next_event {
                if event.timing() > sample_id as u32 {
                    break;
                }
                match event {
                    NoteEvent::NoteOn { note, velocity, .. } => {
                        self.freq_var.set_value(util::midi_note_to_freq(note));
                        self.amp_var.set_value(velocity);
                    }
                    NoteEvent::NoteOff { .. } => {
                        // Note: For a strictly basic example, any NoteOff stops the sound. 
                        // A full synth would verify the `note` matches the currently playing note.
                        self.amp_var.set_value(0.0);
                    }
                    _ => (),
                }
                next_event = context.next_event();
            }

            let gain = self.params.gain.smoothed.next();

            // Tick the fundsp graph forward by exactly one sample
            let (out_l, out_r) = self.graph.get_stereo();

            // Turn the frame into an iterator and write our synth audio safely
            let mut channels = frame.into_iter();
            if let (Some(left), Some(right)) = (channels.next(), channels.next()) {
                *left = out_l * gain;
                *right = out_r * gain;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for SineSynth {
    const CLAP_ID: &'static str = "com.your-domain.sine_synth";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A Basic Sine Wave Synthesizer");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Synthesizer, ClapFeature::Stereo];
}

impl Vst3Plugin for SineSynth {
    // VST3 strictly requires exactly 16 characters for the ID
    const VST3_CLASS_ID: [u8; 16] = *b"SineSynthInstruV";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Instrument, Vst3SubCategory::Synth];
}

nih_export_clap!(SineSynth);
nih_export_vst3!(SineSynth);
