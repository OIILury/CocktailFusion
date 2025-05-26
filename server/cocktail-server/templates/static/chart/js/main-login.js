document.addEventListener("alpine:init", () => {
  Alpine.data("hashtagChart", () => ({
    chart: null,
    init() {
      this.frequences = [
        {
          label: "agriculture",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 15,
            },
            {
              x: "2020-05-19",
              y: 366,
            },
            {
              x: "2020-05-20",
              y: 99,
            },
            {
              x: "2020-05-21",
              y: 14,
            },
            {
              x: "2020-05-22",
              y: 48,
            },
            {
              x: "2020-05-23",
              y: 29,
            },
            {
              x: "2020-05-24",
              y: 9,
            },
            {
              x: "2020-05-25",
              y: 5,
            },
            {
              x: "2020-05-26",
              y: 142,
            },
            {
              x: "2020-05-27",
              y: 58,
            },
          ],
        },
        {
          label: "futureofcap",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 0,
            },
            {
              x: "2020-05-19",
              y: 331,
            },
            {
              x: "2020-05-20",
              y: 89,
            },
            {
              x: "2020-05-21",
              y: 13,
            },
            {
              x: "2020-05-22",
              y: 42,
            },
            {
              x: "2020-05-23",
              y: 22,
            },
            {
              x: "2020-05-24",
              y: 7,
            },
            {
              x: "2020-05-25",
              y: 4,
            },
            {
              x: "2020-05-26",
              y: 1,
            },
            {
              x: "2020-05-27",
              y: 0,
            },
          ],
        },
        {
          label: "alimentation",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 0,
            },
            {
              x: "2020-05-19",
              y: 0,
            },
            {
              x: "2020-05-20",
              y: 0,
            },
            {
              x: "2020-05-21",
              y: 0,
            },
            {
              x: "2020-05-22",
              y: 0,
            },
            {
              x: "2020-05-23",
              y: 1,
            },
            {
              x: "2020-05-24",
              y: 1,
            },
            {
              x: "2020-05-25",
              y: 0,
            },
            {
              x: "2020-05-26",
              y: 68,
            },
            {
              x: "2020-05-27",
              y: 13,
            },
          ],
        },
        {
          label: "goodfoodgoodfarming",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 0,
            },
            {
              x: "2020-05-19",
              y: 325,
            },
            {
              x: "2020-05-20",
              y: 91,
            },
            {
              x: "2020-05-21",
              y: 15,
            },
            {
              x: "2020-05-22",
              y: 42,
            },
            {
              x: "2020-05-23",
              y: 22,
            },
            {
              x: "2020-05-24",
              y: 7,
            },
            {
              x: "2020-05-25",
              y: 2,
            },
            {
              x: "2020-05-26",
              y: 126,
            },
            {
              x: "2020-05-27",
              y: 47,
            },
          ],
        },
        {
          label: "pouruneautrepac",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 0,
            },
            {
              x: "2020-05-19",
              y: 66,
            },
            {
              x: "2020-05-20",
              y: 8,
            },
            {
              x: "2020-05-21",
              y: 11,
            },
            {
              x: "2020-05-22",
              y: 4,
            },
            {
              x: "2020-05-23",
              y: 1,
            },
            {
              x: "2020-05-24",
              y: 0,
            },
            {
              x: "2020-05-25",
              y: 1,
            },
            {
              x: "2020-05-26",
              y: 1,
            },
            {
              x: "2020-05-27",
              y: 20,
            },
          ],
        },
        {
          label: "biodiversité",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 0,
            },
            {
              x: "2020-05-19",
              y: 324,
            },
            {
              x: "2020-05-20",
              y: 149,
            },
            {
              x: "2020-05-21",
              y: 24,
            },
            {
              x: "2020-05-22",
              y: 63,
            },
            {
              x: "2020-05-23",
              y: 30,
            },
            {
              x: "2020-05-24",
              y: 8,
            },
            {
              x: "2020-05-25",
              y: 2,
            },
            {
              x: "2020-05-26",
              y: 0,
            },
            {
              x: "2020-05-27",
              y: 7,
            },
          ],
        },
        {
          label: "europe",
          hidden: false,
          data: [
            {
              x: "2020-05-18",
              y: 0,
            },
            {
              x: "2020-05-19",
              y: 0,
            },
            {
              x: "2020-05-20",
              y: 0,
            },
            {
              x: "2020-05-21",
              y: 0,
            },
            {
              x: "2020-05-22",
              y: 0,
            },
            {
              x: "2020-05-23",
              y: 4,
            },
            {
              x: "2020-05-24",
              y: 1,
            },
            {
              x: "2020-05-25",
              y: 8,
            },
            {
              x: "2020-05-26",
              y: 6,
            },
            {
              x: "2020-05-27",
              y: 4,
            },
          ],
        },
      ];
      this.chart = new Chart(this.$el, {
        parsing: false,
        type: "line",
        data: {
          labels: [],
          datasets: this.frequences.map((f, i) => {
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
        },
      });

      this.chart.canvas.parentNode.style.height = "600px";
      this.chart.canvas.parentNode.style.width = "100%";
    },
    toggleSerie({ hashtag, hidden }) {
      let index = this.frequences.map(({ label }) => label).indexOf(hashtag);

      if (index != -1) Alpine.raw(this.chart)[hidden ? "hide" : "show"](index);
    },
    toggleAll({ hidden }) {
      for (i = 0; i < this.frequences.length; i++) {
        Alpine.raw(this.chart)[hidden ? "hide" : "show"](i);
      }
    },
  }));
});
