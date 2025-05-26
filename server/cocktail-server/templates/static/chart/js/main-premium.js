tweetsChart = null;
hashtagsChart = null;
hashtagsChartHidden = [];
document.addEventListener("alpine:init", () => {
  Alpine.data("hashtagChartPremium", (frequences) => ({
    init() {
      let min =
        frequences.length > 0 && frequences[0].data.length > 0
          ? frequences[0].data[0].x
          : "";
      let max =
        frequences.length > 0 && frequences[0].data.length > 0
          ? frequences[0].data[frequences[0].data.length - 1].x
          : "";

      this.frequences = frequences;
      hashtagsChart = new Chart(this.$el, {
        parsing: false,
        type: "line",
        data: {
          labels: [],
          datasets: this.frequences.map((f) => {
            let { color } = uniqolor(f.label);
            return {
              ...f,
              backgroundColor: color,
              borderColor: color,
            };
          }),
        },
        options: {
          animation: false,
          plugins: {
            legend: {
              display: false,
            },
          },
          maintainAspectRatio: false,
          scales: {
            y: {
              title: {
                display: true,
                text: this.$el.getAttribute("data-label"),
              },
              beginAtZero: true,
              ticks: {
                precision: 0,
              },
              grace: "5%",
            },
            x: {
              type: "time",
              time: {
                unit: "day",
                displayFormats: "d/mm/Y",
              },
              min: min,
              max: max,
            },
          },
          onClick: (evt, el, chart) => {
            let date =
              chart.data.datasets[el[0]?.datasetIndex]?.data[el[0].index].x;
            let hashtag = chart.data.datasets[el[0]?.datasetIndex]?.label;

            if (date != undefined && hashtag != undefined) {
              window.location.replace(
                document
                  .getElementById("hashtags-chart-premium")
                  .getAttribute("data-result-path") +
                  "?date=" +
                  date +
                  "&hashtag=" +
                  hashtag
              );
            }
          },
        },
      });

      hashtagsChart.canvas.parentNode.style.height = "400px";
    },
    toggleSerie({ hashtag, hidden }) {
      let index = this.frequences.map(({ label }) => label).indexOf(hashtag);

      if (index != -1) {
        Alpine.raw(hashtagsChart)[hidden ? "hide" : "show"](index);

        if (hidden) {
          hashtagsChartHidden.push(index);
        } else {
          hashtagsChartHidden = hashtagsChartHidden.filter((i) => i != index);
        }
      }
    },
    toggleAll({ hidden }) {
      for (i = 0; i < this.frequences.length; i++) {
        Alpine.raw(hashtagsChart)[hidden ? "hide" : "show"](i);

        if (hidden) {
          hashtagsChartHidden.push(i);
        }
      }

      if (!hidden) {
        hashtagsChartHidden = [];
      }
    },
  }));

  Alpine.data("tweetsChartPremium", (initialData) => ({
    init() {
      let { color } = uniqolor("Tweets");
      this.data = initialData;
      tweetsChart = new Chart(this.$el, {
        parsing: false,
        type: "line",
        data: {
          labels: [],
          datasets: [
            {
              ...this.data,
              borderColor: color,
              borderColor: color,
            },
          ],
        },
        options: {
          animation: false,
          plugins: {
            legend: {
              display: false,
            },
          },
          maintainAspectRatio: false,
          scales: {
            y: {
              title: {
                display: true,
                text: this.$el.getAttribute("data-label"),
              },
              beginAtZero: true,
              ticks: {
                precision: 0,
              },
              grace: "5%",
            },
            x: {
              type: "time",
              time: {
                unit: "day",
                displayFormats: "d/mm/Y",
              },
            },
          },
          onClick: (evt, el, chart) => {
            let date =
              chart.data.datasets[el[0]?.datasetIndex]?.data[el[0].index].x;

            if (date != undefined) {
              window.location.replace(
                document
                  .getElementById("tweets-chart-premium")
                  .getAttribute("data-result-path") +
                  "?date=" +
                  date
              );
            }
          },
        },
      });

      tweetsChart.canvas.parentNode.style.height = "400px";
    },
  }));
});

function update_periodicity_tweets() {
  if (tweetsChart === null) {
    return;
  }
  let periodicity = document.getElementById("displayBy").value;
  let data = JSON.parse(
    document.getElementById("tweets-chart-premium").getAttribute("data-json")
  );
  let { color } = uniqolor("Tweets");

  if (periodicity === "jour") {
    tweetsChart.data.datasets = [
      {
        ...data,
        borderColor: color,
        borderColor: color,
      },
    ];
    tweetsChart.options.scales.x.time.unit = "day";
    tweetsChart.update();

    return;
  }

  let finalData = { data: [] };
  if (periodicity === "semaine") {
    data.data.forEach((value) => {
      if (
        (index = finalData.data.findIndex(
          (i) =>
            luxon.DateTime.fromISO(i.x).toFormat("yyyy-W") ===
            luxon.DateTime.fromISO(value.x).toFormat("yyyy-W")
        )) != -1
      ) {
        finalData.data[index].y = finalData.data[index].y + value.y;
      } else {
        finalData.data.push(value);
      }
    });
    tweetsChart.options.scales.x.time.unit = "week";
  } else if (periodicity === "mois") {
    data.data.forEach((value) => {
      if (
        (index = finalData.data.findIndex(
          (i) =>
            luxon.DateTime.fromISO(i.x).toFormat("yyyy-M") ===
            luxon.DateTime.fromISO(value.x).toFormat("yyyy-M")
        )) != -1
      ) {
        finalData.data[index].y = finalData.data[index].y + value.y;
      } else {
        finalData.data.push(value);
      }
    });
    tweetsChart.options.scales.x.time.unit = "month";
  } else if (periodicity === "annee") {
    data.data.forEach((value) => {
      if (
        (index = finalData.data.findIndex(
          (i) =>
            luxon.DateTime.fromISO(i.x).toFormat("yyyy") ===
            luxon.DateTime.fromISO(value.x).toFormat("yyyy")
        )) != -1
      ) {
        finalData.data[index].y = finalData.data[index].y + value.y;
      } else {
        finalData.data.push(value);
      }
    });
    tweetsChart.options.scales.x.time.unit = "year";
  }
  tweetsChart.data.datasets = [
    {
      ...finalData,
      borderColor: color,
      borderColor: color,
    },
  ];

  tweetsChart.update();
}

function update_periodicity_hashtags() {
  if (hashtagsChart === null) {
    return;
  }
  let periodicity = document.getElementById("displayBy").value;
  let data = JSON.parse(
    document.getElementById("hashtags-chart-premium").getAttribute("data-json")
  );

  if (periodicity === "jour") {
    hashtagsChart.data.datasets = data.map((f) => {
      let { color } = uniqolor(f.label);
      return {
        ...f,
        backgroundColor: color,
        borderColor: color,
      };
    });
    hashtagsChart.options.scales.x.time.unit = "day";
    hashtagsChart.update();

    hashtagsChartHidden.forEach((i) => {
      Alpine.raw(hashtagsChart)["hide"](i);
    });

    return;
  }

  let finalData = [];
  data.forEach((hashtags) => {
    finalData[finalData.length] = {
      label: hashtags.label,
      hidden: false,
      data: [],
    };

    let hashtagIndex = finalData.length - 1;
    if (periodicity === "semaine") {
      hashtags.data.forEach((value) => {
        if (
          (index = finalData[hashtagIndex].data.findIndex(
            (i) =>
              luxon.DateTime.fromISO(i.x).toFormat("yyyy-W") ===
              luxon.DateTime.fromISO(value.x).toFormat("yyyy-W")
          )) != -1
        ) {
          finalData[hashtagIndex].data[index].y =
            finalData[hashtagIndex].data[index].y + value.y;
        } else {
          finalData[hashtagIndex].data.push(value);
        }
      });
      hashtagsChart.options.scales.x.time.unit = "week";
    } else if (periodicity === "mois") {
      hashtags.data.forEach((value) => {
        if (
          (index = finalData[hashtagIndex].data.findIndex(
            (i) =>
              luxon.DateTime.fromISO(i.x).toFormat("yyyy-M") ===
              luxon.DateTime.fromISO(value.x).toFormat("yyyy-M")
          )) != -1
        ) {
          finalData[hashtagIndex].data[index].y =
            finalData[hashtagIndex].data[index].y + value.y;
        } else {
          finalData[hashtagIndex].data.push(value);
        }
      });
      hashtagsChart.options.scales.x.time.unit = "month";
    } else if (periodicity === "annee") {
      hashtags.data.forEach((value) => {
        if (
          (index = finalData[hashtagIndex].data.findIndex(
            (i) =>
              luxon.DateTime.fromISO(i.x).toFormat("yyyy") ===
              luxon.DateTime.fromISO(value.x).toFormat("yyyy")
          )) != -1
        ) {
          finalData[hashtagIndex].data[index].y =
            finalData[hashtagIndex].data[index].y + value.y;
        } else {
          finalData[hashtagIndex].data.push(value);
        }
      });
      hashtagsChart.options.scales.x.time.unit = "year";
    }
  });
  hashtagsChart.data.datasets = finalData.map((f) => {
    let { color } = uniqolor(f.label);
    return {
      ...f,
      backgroundColor: color,
      borderColor: color,
    };
  });

  hashtagsChart.update();

  hashtagsChartHidden.forEach((i) => {
    Alpine.raw(hashtagsChart)["hide"](i);
  });
}
