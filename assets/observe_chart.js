(function () {
  const width = 800;
  const height = 600;
  const margin = 50;

  const x = d3
    .scaleLinear()
    .domain([0, 100]) // These values will be replaced.
    .range([margin, width - margin]);
  const y = d3
    .scaleLinear()
    .domain([0, 20]) // These values are completely arbitrary.
    .range([height - margin, margin]);
  const svg = d3
    .create("svg")
    .attr("id", "measurement")
    .attr("width", width)
    .attr("height", height)
    .attr("viewBox", [0, 0, width, height])
    .attr("style", "max-width: 100%; height: auto; height: intrinsic;");
  const line = d3
    .line()
    .x((d) => x(d.x))
    .y((d) => y(d.y));
  // x-axis
  xAxis = svg
    .append("g")
    .attr("transform", `translate(0,${height - margin})`)
    .call(
      d3
        .axisBottom(x)
        .ticks(width / 160)
        .tickSizeOuter(0),
    );
  // y-axis
  svg
    .append("g")
    .attr("transform", `translate(${margin},0)`)
    .call(d3.axisLeft(y).ticks(height / 80))
    .call((g) =>
      g
        .append("text")
        .attr("x", -margin)
        .attr("y", 10)
        .attr("text-anchor", "start")
        .text("Amplitude"),
    );
  svg
    .append("path")
    .attr("class", "line")
    .attr("fill", "none")
    .attr("stroke", "steelblue")
    .attr("stroke-width", 1.5);
  document.currentScript.parentElement.appendChild(svg.node());
  // There can be a socket already here if the page is refetched by htmx.
  if (window.spectrumSocket) {
    window.spectrumSocket.close();
  }
  window.spectrumSocket = new WebSocket("/telescope/fake/spectrum");
  window.spectrumSocket.onmessage = async (event) => {
    let dataView = new DataView(await event.data.arrayBuffer());
    let data = [];
    // The data is interleaved (freq, spectrum).
    for (let i = 0; i < dataView.byteLength; i += 16) {
      data.push({
        // Convert to MHz immediately for display.
        x: dataView.getFloat64(i, true) / 1e6,
        y: dataView.getFloat64(i + 8, true),
      });
    }
    const frequency_range = d3.extent(data, (d) => d.x);
    x.domain(frequency_range);
    xAxis.call(d3.axisBottom(x));
    const line = d3
      .line()
      .x((d) => x(d.x))
      .y((d) => y(d.y));
    svg.select(".line").datum(data).attr("d", line);
  };
})();
