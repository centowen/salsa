use std::env::set_var;
use anyhow::{Result};
use num_complex::Complex;
use uhd::{self, StreamCommand, StreamCommandType, StreamTime, TuneRequest, Usrp};
use rustfft::{FftPlanner, Fft, FftDirection, algorithm::Radix4};
use plotters::prelude::*;
use median::Filter;

const CHANNEL: usize = 0; // USRP input channel
const TINT: usize = 4; // integration time, seconds
const SAMP_RATE: f64 = 2.5e6 ; // sample rate, Hz
const NUM_SAMPLES: usize = TINT * SAMP_RATE as usize; // total number of samples expected
const FFT_POINTS: usize = 4096; // ^2 Number of points in FFT, setting spectral resolution
const AVG_POINTS: usize = 512; // ^2 Number of points after average, setting spectral resolution
const NUM_STACK: usize = NUM_SAMPLES / FFT_POINTS;
const NUM_AVG: usize = FFT_POINTS / AVG_POINTS;
const GAIN: f64 = 40.0;
const CFREQ: f64 = 1.4204e9;


fn rec_info(usrp: Usrp) {
    let sr = usrp.get_rx_sample_rates(CHANNEL).unwrap();
    print!("{:?}",sr);

    let bw = usrp.get_rx_bandwidth_range(CHANNEL).unwrap();
    print!("{:?}",bw);
    
    let fr = usrp.get_rx_frequency_range(CHANNEL).unwrap();
    print!("{:?}",fr);
    
    let gains = usrp.get_rx_gain_names(CHANNEL).unwrap();
    print!("{:?}",gains);
    
    let gain = usrp.get_rx_gain(CHANNEL, "").unwrap();
    print!("{:?}",gain);
}

pub fn main() -> Result<()> {
    set_var("RUST_LOG", "DEBUG");
    env_logger::init();

    //let mut usrp = Usrp::open("addr=192.168.5.32").unwrap();// Vale
    let mut usrp = Usrp::open("addr=192.168.5.31").unwrap(); // Brage
    
    usrp.set_rx_sample_rate(SAMP_RATE, CHANNEL)?;
    usrp.set_rx_frequency(&TuneRequest::with_frequency(CFREQ), CHANNEL)?;
    usrp.set_rx_gain(GAIN, CHANNEL, "")?; // empty string should mean all gains
    usrp.set_rx_antenna("TX/RX", CHANNEL)?;
    usrp.set_rx_dc_offset_enabled(true, CHANNEL)?;

    let mut receiver = usrp
        .get_rx_stream(&uhd::StreamArgs::<Complex<i16>>::new("sc16"))
        .unwrap();

    let mut buffer = uhd::alloc_boxed_slice::<Complex<i16>, NUM_SAMPLES>();
    
    receiver.send_command(&StreamCommand {
        command_type: StreamCommandType::CountAndDone(buffer.len() as u64),
        time: StreamTime::Now,
    })?;
    log::info!("Done setting up...");
    let status = receiver.receive_simple(buffer.as_mut())?;

    log::info!("{:?}", status);
    //log::info!("{:?}", &buffer[..16]);

    log::info!("Doing FFT...");
    // setup fft
    //let fft = Radix4::new(FFT_POINTS, FftDirection::Forward);
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(FFT_POINTS);
    // array to store power spectrum (abs of FFT result)
    let mut fft_abs: [f64; FFT_POINTS] = [0.0; FFT_POINTS];
    // Loop through the samples, taking FFF_POINTS each time
    for n in 0..NUM_STACK {
        let mut fft_buffer: Vec<Complex<f64>> = buffer[n*FFT_POINTS..(n+1)*FFT_POINTS].iter().copied()
            .map(|x| {
                Complex::<f64>::new(x.re as f64, x.im as f64)
            }).collect();
        // Do the FFT
        fft.process(&mut fft_buffer);
        // Add absolute values to stacked spectrum
        // Seems the pos/neg halves of spectrum are flipped, so reflip them
        for i in 0..FFT_POINTS/2 {
            fft_abs[i+FFT_POINTS/2] = fft_abs[i+FFT_POINTS/2] + fft_buffer[i].norm();
            fft_abs[i] = fft_abs[i] + fft_buffer[i+FFT_POINTS/2].norm();
        }
    }
    log::info!("Normalise...");
    let mut ymax : f64 = 0.0;
    let mut ymin : f64 = 0.0;
    // Normalise spectrum by number of stackings,
    // do **2 to get power spectrum, and median filter
    // also lot max/min for plotting
    let mut filter = Filter::new(21);
    for i in 0..FFT_POINTS {
        fft_abs[i] = fft_abs[i]*fft_abs[i] / ( NUM_STACK as f64);
        fft_abs[i] = filter.consume(fft_abs[i]);

        if fft_abs[i] > ymax {
            ymax = fft_abs[i];
        }
        if fft_abs[i] < ymin {
            ymin = fft_abs[i];
        }
    }
    
    // Average spectrum to save data
    let mut fft_avg: [f64; AVG_POINTS] = [0.0; AVG_POINTS];
    for i in 0..AVG_POINTS {
        let mut avg = 0.0;
            for j in NUM_AVG*i..NUM_AVG*(i+1) {
                avg = avg+fft_abs[j];
            }
        fft_avg[i] = avg/(NUM_AVG as f64);
    }

    //let mut freqs: [f64; FFT_POINTS] = [0.0; FFT_POINTS];

    log::info!("Plot...");
    let root_area = BitMapBackend::new("plot.png", (800, 600)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root_area)
                    //.build_cartesian_2d(0..FFT_POINTS, ymin..ymax).unwrap();
                    .build_cartesian_2d(0..AVG_POINTS, ymin..ymax).unwrap();
    chart
        .configure_mesh()
        .x_labels(3)
        .y_labels(3)
        .draw()?;
    chart
        //.draw_series(LineSeries::new(fft_abs.iter().copied().enumerate(),RED)).unwrap();
        .draw_series(LineSeries::new(fft_avg.iter().copied().enumerate(),RED)).unwrap();

    Ok(())

}
