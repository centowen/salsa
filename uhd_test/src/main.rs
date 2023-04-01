use anyhow::Result;
use median::Filter;
use num_complex::Complex;
use plotters::prelude::*;
use rustfft::FftPlanner;
use std::env::set_var;
use uhd::{self, StreamCommand, StreamCommandType, StreamTime, TuneRequest, Usrp};

//fn rec_info(usrp: Usrp) {
//    let sr = usrp.get_rx_sample_rates(CHANNEL).unwrap();
//    print!("{:?}",sr);
//
//    let bw = usrp.get_rx_bandwidth_range(CHANNEL).unwrap();
//    print!("{:?}",bw);
//
//    let fr = usrp.get_rx_frequency_range(CHANNEL).unwrap();
//    print!("{:?}",fr);
//
//    let gains = usrp.get_rx_gain_names(CHANNEL).unwrap();
//    print!("{:?}",gains);
//
//    let gain = usrp.get_rx_gain(CHANNEL, "").unwrap();
//    print!("{:?}",gain);
//}

fn measure_switched(
    usrp: &mut Usrp,
    sfreq: f64,
    rfreq: f64,
    fft_pts: usize,
    tint: f64,
    avg_pts: usize,
    srate: f64,
    spec: &mut Vec<f64>,
) {
    let mut spec_sig: Vec<f64> = vec![];
    measure(
        usrp,
        sfreq,
        fft_pts,
        0.5 * tint,
        avg_pts,
        srate,
        &mut spec_sig,
    );
    let mut spec_ref: Vec<f64> = vec![];
    measure(
        usrp,
        rfreq,
        fft_pts,
        0.5 * tint,
        avg_pts,
        srate,
        &mut spec_ref,
    );
    for i in 0..avg_pts {
        spec.push(0.5 * (spec_sig[i] - spec_ref[i]));
    }
}

fn measure(
    usrp: &mut Usrp,
    cfreq: f64,
    fft_pts: usize,
    tint: f64,
    avg_pts: usize,
    srate: f64,
    fft_avg: &mut Vec<f64>,
) {
    let nsamp: f64 = tint * srate; // total number of samples to request
    let nstack: usize = (nsamp as usize) / fft_pts;
    let navg: usize = fft_pts / avg_pts;

    usrp.set_rx_frequency(&TuneRequest::with_frequency(cfreq), 0)
        .unwrap(); // The N210 only has one input channel 0.

    let mut receiver = usrp
        .get_rx_stream(&uhd::StreamArgs::<Complex<i16>>::new("sc16"))
        .unwrap();

    let mut buffer = vec![Complex::<i16>::default(); nsamp as usize];

    receiver
        .send_command(&StreamCommand {
            command_type: StreamCommandType::CountAndDone(buffer.len() as u64),
            time: StreamTime::Now,
        })
        .unwrap();
    receiver.receive_simple(buffer.as_mut()).unwrap();

    log::info!("Doing FFT...");
    // array to store power spectrum (abs of FFT result)
    let mut fft_abs: Vec<f64> = Vec::with_capacity(fft_pts);
    fft_abs.resize(fft_pts, 0.0);
    // setup fft
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(fft_pts);
    // Loop through the samples, taking FFF_POINTS each time
    for n in 0..nstack {
        let mut fft_buffer: Vec<Complex<f64>> = buffer[n * fft_pts..(n + 1) * fft_pts]
            .iter()
            .copied()
            .map(|x| Complex::<f64>::new(x.re as f64, x.im as f64))
            .collect();
        // Do the FFT
        fft.process(&mut fft_buffer);
        // Add absolute values to stacked spectrum
        // Seems the pos/neg halves of spectrum are flipped, so reflip them
        for i in 0..fft_pts / 2 {
            fft_abs[i + fft_pts / 2] = fft_abs[i + fft_pts / 2] + fft_buffer[i].norm();
            fft_abs[i] = fft_abs[i] + fft_buffer[i + fft_pts / 2].norm();
        }
    }
    log::info!("Normalise...");
    // Normalise spectrum by number of stackings,
    // do **2 to get power spectrum, and median filter
    // also lot max/min for plotting
    let mut filter = Filter::new(21);
    for i in 0..fft_pts {
        fft_abs[i] = fft_abs[i] * fft_abs[i] / (nstack as f64);
        fft_abs[i] = filter.consume(fft_abs[i]);
    }

    // Average spectrum to save data
    for i in 0..avg_pts {
        let mut avg = 0.0;
        for j in navg * i..navg * (i + 1) {
            avg = avg + fft_abs[j];
        }
        fft_avg.push(avg / (navg as f64));
    }
}

pub fn main() -> Result<()> {
    set_var("RUST_LOG", "DEBUG");
    env_logger::init();

    let fft_pts: usize = 4096; // ^2 Number of points in FFT, setting spectral resolution
    let avg_pts: usize = 512; // ^2 Number of points after average, setting spectral resolution
    let gain: f64 = 40.0;

    //let mut usrp = Usrp::open("addr=192.168.5.32").unwrap();// Vale
    let mut usrp = Usrp::open("addr=192.168.5.31").unwrap(); // Brage
                                                             // The N210 only has one input channel 0.
    usrp.set_rx_gain(gain, 0, "")?; // empty string to set all gains
    usrp.set_rx_antenna("TX/RX", 0)?;
    usrp.set_rx_dc_offset_enabled(true, 0)?;

    // Switched HI example
    let tint: f64 = 11.0; // integration time, seconds
    let srate: f64 = 2.5e6; // sample rate, Hz
    let sfreq: f64 = 1.4204e9;
    let rfreq: f64 = 1.4179e9;
    usrp.set_rx_sample_rate(srate as f64, 0)?;
    let tcyc: usize = 4 ; // time per integration cycle
    let ncyc = (tint as usize) / (tcyc);
    let rest = tint % (tcyc as f64);
    let mut spec: Vec<f64> = vec![];
    for c in 1..=ncyc {
        log::info!("Cycle switch measurement...");
        measure_switched(
            &mut usrp, sfreq, rfreq, fft_pts, tcyc as f64, avg_pts, srate, &mut spec,
        );
    }
    // Assume tint in integer seconds, so if there is a rest it is > 0.5
    if rest > 0.5 {
        log::info!("Last rest switch measurement...");
        measure_switched(
            &mut usrp, sfreq, rfreq, fft_pts, rest, avg_pts, srate, &mut spec,
        );
    }
    
    // Non switched GNSS example
    //let tint: f64 = 1.0; // integration time, seconds
    //let srate: f64 = 25e6; // sample rate, Hz
    //let sfreq: f64 = 1.5754e9;
    //let mut spec: Vec<f64> = vec![];
    //usrp.set_rx_sample_rate(srate as f64, 0)?;
    //measure(
    //    &mut usrp, sfreq, fft_pts, tint, avg_pts, srate, &mut spec,
    //);

    let mut ymax: f64 = 0.0;
    let mut ymin: f64 = 0.0;
    for i in 0..avg_pts {
        if spec[i] > ymax {
            ymax = spec[i];
        }
        if spec[i] < ymin {
            ymin = spec[i];
        }
    }

    log::info!("Plot...");
    //let mut freqs: [f64; fft_pts] = [0.0; fft_pts];
    let root_area = BitMapBackend::new("plot.png", (800, 600)).into_drawing_area();
    root_area.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root_area)
        .build_cartesian_2d(0..avg_pts, ymin..ymax)
        .unwrap();
    chart.configure_mesh().x_labels(3).y_labels(3).draw()?;
    chart
        .draw_series(LineSeries::new(spec.iter().copied().enumerate(), RED))
        .unwrap();

    Ok(())
}
