use plotters::prelude::*;
use plotters_canvas::CanvasBackend;
use yew::prelude::*;

#[derive(PartialEq, Properties)]
pub struct GraphProperties {
    pub id: AttrValue,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
}

pub enum Message {
    DrawGraph,
}

#[derive(Debug)]
pub enum DrawError {
    IncorrectInputData,
    PlotterError,
}

impl std::fmt::Display for DrawError {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        return match self {
            Self::IncorrectInputData => write!(fmt, "Incorrect input data to graph"),
            Self::PlotterError => write!(fmt, "Plotter error"),
        };
    }
}

pub struct Graph {
    pub draw_result: Option<Result<(), DrawError>>,
    pub x: Option<Vec<f64>>,
    pub y: Option<Vec<f64>>,
}

fn draw_graph<DB: DrawingBackend>(
    backend: DB,
    x: &Vec<f64>,
    y: &Vec<f64>,
) -> Result<(), DrawError> {
    if x.len() != y.len() || x.is_empty() || y.is_empty() {
        return Err(DrawError::IncorrectInputData);
    }
    // scale data for plotting
    let x: Vec<f64> = x.iter().map(|a| a / 1.0e6).collect();

    let root = backend.into_drawing_area();
    root.fill(&WHITE).map_err(|_| DrawError::PlotterError)?;
    let x_min = *x.first().expect("x should not be empty");
    let x_max = *x.last().expect("x should not be empty");
    let y_min = y.iter().fold(f64::INFINITY, |a, &b| a.min(b));
    let y_max = y.iter().fold(-f64::INFINITY, |a, &b| a.max(b));
    log::info!("xmin {}, xmax {}", x_min, x_max);
    log::info!("ymin {}, ymax {}", y_min, y_max);

    let mut chart = ChartBuilder::on(&root)
        .margin(10)
        .x_label_area_size(50)
        .y_label_area_size(60)
        .build_cartesian_2d(x_min..x_max, y_min..y_max)
        .map_err(|_| DrawError::PlotterError)?;
    chart
        .configure_mesh()
        .x_labels(5)
        .y_labels(5)
        .x_desc("Frequency [MHz]")
        .y_desc("Intensity")
        .axis_desc_style(("sans-serif", 15))
        .draw()
        .map_err(|_| DrawError::PlotterError)?;
    chart
        .draw_series(LineSeries::new(
            x.iter().cloned().zip(y.iter().cloned()),
            &BLUE,
        ))
        .map_err(|_| DrawError::PlotterError)?;
    root.present().map_err(|_| DrawError::PlotterError)?;

    Ok(())
}

impl Component for Graph {
    type Message = Message;
    type Properties = GraphProperties;

    fn create(_ctx: &Context<Self>) -> Self {
        Graph {
            draw_result: None,
            x: None,
            y: None,
        }
    }

    fn update(&mut self, ctx: &Context<Self>, msg: Self::Message) -> bool {
        match msg {
            Self::Message::DrawGraph => {
                let backend = CanvasBackend::new(ctx.props().id.as_str())
                    .expect("Could not attach to canvas");
                let x = ctx.props().x.clone();
                let y = ctx.props().y.clone();
                self.draw_result = Some(draw_graph(backend, &x, &y));
                self.x = Some(x);
                self.y = Some(y);
            }
        }
        true
    }

    fn view(&self, ctx: &Context<Self>) -> Html {
        if self.x.as_ref() != Some(&ctx.props().x) || self.y.as_ref() != Some(&ctx.props().y) {
            log::info!("x: {:?}", &self.x);
            log::info!("new x: {:?}", &ctx.props().x);
            ctx.link().send_message(Message::DrawGraph);
        }

        html! {
            <canvas width="400" height="300" id={ctx.props().id.clone()} />
        }
    }
}
