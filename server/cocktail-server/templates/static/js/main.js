/* AU CHARGEMENT + RESIZE */
window.addEventListener("load", onLoadFunction);
window.addEventListener("turbo:render", onLoadFunction);

document.addEventListener("turbo:before-stream-render", function (event) {
  event.preventDefault();
  event.target.performAction();
  loadTweets();
});
function onLoadFunction(e) {
  loadTweets();
  loadCommunities();
  /*DROPDOWN*/
  var acc = document.getElementsByClassName("accordion");
  var i;

  for (i = 0; i < acc.length; i++) {
    acc[i].addEventListener("click", function () {
      this.classList.toggle("active");
      var panel = this.nextElementSibling;
      if (panel.style.display === "block") {
        panel.style.display = "none";
      } else {
        panel.style.display = "block";
      }
    });
  }

  /* LEGENDE PAGE GRAPHIQUE*/
  /* Récupère les hauteurs vers une variable css pour déduire de 100vh */
  try {
    var staticHeightLegend =
      document.querySelector(".header-site").offsetHeight +
      document.querySelector(".hashtags-legend-head").offsetHeight +
      document.querySelector(".footer-site").offsetHeight;
    document.documentElement.style.setProperty(
      "--staticHeightLegend",
      staticHeightLegend + "px"
    );
  } catch (error) {}
}

function loadTweets() {
  let tweets = document.getElementsByClassName("tweet-info");

  for (i = 0; i < tweets.length; i++) {
    var id = tweets[i].getAttribute("data-tweeet-id");
    var source = tweets[i].getAttribute("data-tweet-source");
    const j = i;

    if (source === "Bluesky") {
      // Pour les tweets Bluesky, créer un affichage personnalisé
      createBlueskyTweet(tweets[j]);
    } else {
      // Pour les tweets Twitter, utiliser l'API Twitter
      twttr.widgets.createTweet(id, tweets[i]).then((res) => {
        if (res) {
          tweets[j].getElementsByClassName("retweet-container")[0].style.display =
            "flex";
        } else {
          tweets[j].getElementsByClassName("tweet-erreur")[0].style.display =
            "block";
        }
      });
    }
  }
}

function createBlueskyTweet(tweetElement) {
  const id = tweetElement.getAttribute("data-tweeet-id");
  const text = tweetElement.getAttribute("data-tweet-text");
  const author = tweetElement.getAttribute("data-tweet-author");
  const authorName = tweetElement.getAttribute("data-tweet-author-name");
  const date = tweetElement.getAttribute("data-tweet-date");
  
  // Sauvegarder le contenu existant (conteneur de retweet, etc.)
  const existingContent = tweetElement.innerHTML;
  
  // Créer l'HTML personnalisé pour le tweet Bluesky
  const tweetHTML = `
    <div class="bluesky-tweet">
      <div class="bluesky-tweet-header">
        <div class="bluesky-tweet-author">
          <div class="bluesky-tweet-author-name">${escapeHtml(authorName || author)}</div>
          <div class="bluesky-tweet-author-handle">@${escapeHtml(author)}</div>
          <div class="bluesky-tweet-date">${formatDate(date)}</div>
        </div>
        <div class="bluesky-tweet-logo">
          <svg width="20" height="20" viewBox="0 0 24 24" fill="#1185fe">
            <path d="M12 2C6.48 2 2 6.48 2 12s4.48 10 10 10 10-4.48 10-10S17.52 2 12 2zm-2 15l-5-5 1.41-1.41L10 14.17l7.59-7.59L19 8l-9 9z"/>
          </svg>
        </div>
      </div>
      <div class="bluesky-tweet-content">
        ${formatTweetText(text)}
      </div>
      <div class="bluesky-tweet-footer">
        <a href="https://bsky.app/profile/${escapeHtml(author)}/post/${escapeHtml(id.split('/').pop())}" target="_blank" class="bluesky-tweet-link">
          Voir sur Bluesky
        </a>
      </div>
    </div>
  `;
  
  // Ajouter le tweet Bluesky après le contenu existant
  tweetElement.innerHTML = existingContent + tweetHTML;
  
  // Afficher le conteneur de retweet s'il existe
  const retweetContainer = tweetElement.getElementsByClassName("retweet-container")[0];
  if (retweetContainer) {
    retweetContainer.style.display = "flex";
  }
}

function escapeHtml(text) {
  if (!text) return '';
  const div = document.createElement('div');
  div.textContent = text;
  return div.innerHTML;
}

function formatDate(dateString) {
  try {
    const date = new Date(dateString);
    return date.toLocaleDateString('fr-FR', {
      year: 'numeric',
      month: 'short',
      day: 'numeric',
      hour: '2-digit',
      minute: '2-digit'
    });
  } catch (e) {
    return dateString;
  }
}

function formatTweetText(text) {
  if (!text) return '';
  
  // Remplacer les URLs par des liens
  text = text.replace(/(https?:\/\/[^\s]+)/g, '<a href="$1" target="_blank" rel="noopener">$1</a>');
  
  // Remplacer les hashtags
  text = text.replace(/#(\w+)/g, '<span class="hashtag">#$1</span>');
  
  // Remplacer les mentions
  text = text.replace(/@(\w+)/g, '<span class="mention">@$1</span>');
  
  return text;
}

let renderer;

async function saveAsPNG(tab, community, centrality) {
  if (renderer == undefined) {
    return;
  }
  let inputLayers = ["edges", "nodes", "edgeLabels", "labels"];
  const { width, height } = renderer.getDimensions();

  // This pixel ratio is here to deal with retina displays.
  // Indeed, for dimensions W and H, on a retina display, the canvases
  // dimensions actually are 2 * W and 2 * H. Sigma properly deals with it, but
  // we need to readapt here:
  const pixelRatio = window.devicePixelRatio || 1;

  const tmpRoot = document.createElement("DIV");
  tmpRoot.style.width = `${width}px`;
  tmpRoot.style.height = `${height}px`;
  tmpRoot.style.position = "absolute";
  tmpRoot.style.right = "101%";
  tmpRoot.style.bottom = "101%";
  document.body.appendChild(tmpRoot);

  // Instantiate sigma:
  const tmpRenderer = new Sigma(
    renderer.getGraph(),
    tmpRoot,
    renderer.getSettings()
  );

  // Copy camera and force to render now, to avoid having to wait the schedule /
  // debounce frame:
  tmpRenderer.getCamera().setState(renderer.getCamera().getState());
  tmpRenderer.refresh();

  // Create a new canvas, on which the different layers will be drawn:
  const canvas = document.createElement("CANVAS");
  canvas.setAttribute("width", width * pixelRatio + "");
  canvas.setAttribute("height", height * pixelRatio + "");
  const ctx = canvas.getContext("2d");

  // Draw a white background first:
  ctx.fillStyle = "#fff";
  ctx.fillRect(0, 0, width * pixelRatio, height * pixelRatio);

  // For each layer, draw it on our canvas:
  const canvases = tmpRenderer.getCanvases();
  const layers = inputLayers
    ? inputLayers.filter((id) => !!canvases[id])
    : Object.keys(canvases);
  layers.forEach((id) => {
    ctx.drawImage(
      canvases[id],
      0,
      0,
      width * pixelRatio,
      height * pixelRatio,
      0,
      0,
      width * pixelRatio,
      height * pixelRatio
    );
  });

  var link = document.createElement("a");
  link.download = "export_" + tab + "_" + community + "_" + centrality + ".png";

  // Save the canvas as a PNG image:
  canvas.toBlob((blob) => {
    link.href = URL.createObjectURL(blob);
    link.click();

    // Cleanup:
    tmpRenderer.kill();
    tmpRoot.remove();
  }, "image/png");
}

function loadCommunities() {
  let nodesEl = document.getElementById("data-nodes");
  let edgesEl = document.getElementById("data-edges");

  if (!nodesEl || !edgesEl) {
    return;
  }

  let nodes = JSON.parse(nodesEl.getAttribute("data-json"));
  let edges = JSON.parse(edgesEl.getAttribute("data-json"));

  // adding nodes and edges to the graph
  let dataset = { nodes: nodes, edges: edges };

  let container, graph;

  const state = {};

  const selectNode = (node) => {
    $("ul.list-membre").empty();
    $(".returntext").hide();
    if (state.selectedNode === node) {
      node = null;
    }
    if (node) {
      state.selectedNode = node;
      state.selectedNeighbors = new Set(graph.neighbors(node));
      $(".returntext").show();
      $(".title-connexion").html(`Les connexions de ${node}`);
    } else {
      $(".title-connexion").html(`Communauté complète`);
      state.selectedNode = undefined;
      state.selectedNeighbors = undefined;
    }

    renderer.refresh();

    var f = [];

    if (state.selectedNode !== undefined) {
      graph.forEachNeighbor(
        state.selectedNode,
        function (neighbor, attributes) {
          f.push(`<li class="membership"
          id="user-${neighbor}"
    >
        <span 
            class="name-twitter"
        >${neighbor}</span>
        <span class="id-twitter">@${neighbor}</span>
        <a class="link-twitter" href="https://twitter.com/${neighbor}" target="_blank">voir sur twitter</a>
    </li>`);
        }
      );
      $("ul.list-membre").html(f.join(""));

      graph.forEachNeighbor(
        state.selectedNode,
        function (neighbor, attributes) {
          $(`#user-${neighbor}`).click(() => {
            selectNode(neighbor);
          });
        }
      );
    }
  };

  $(".returntext").hide();
  container = document.getElementById("sigma-container");

  graph = new graphology.Graph();

  dataset.nodes.forEach((node) =>
    graph.addNode(node.id, {
      ...node,
    })
  );
  dataset.edges.forEach((edge) =>
    graph.addEdge(edge.from, edge.to, { ...edge })
  );

  const scores = graph
    .nodes()
    .map((node) => graph.getNodeAttribute(node, "size"));
  const minDegree = Math.min(...scores);
  const maxDegree = Math.max(...scores);
  const MIN_NODE_SIZE = 1;
  const MAX_NODE_SIZE = 7;
  graph.forEachNode((node) => {
    graph.setNodeAttribute(
      node,
      "size",
      ((graph.getNodeAttribute(node, "size") - minDegree) /
        (maxDegree - minDegree)) *
        (MAX_NODE_SIZE - MIN_NODE_SIZE) +
        MIN_NODE_SIZE
    );
  });

  const scoresEdge = graph
    .edges()
    .map((edge) => graph.getEdgeAttribute(edge, "size"));
  const minDegreeEdge = Math.min(...scoresEdge);
  const maxDegreeEdge = Math.max(...scoresEdge);
  const MIN_EDGE_SIZE = 0.2;
  const MAX_EDGE_SIZE = 0.5;
  graph.forEachEdge((edge) => {
    graph.setEdgeAttribute(
      edge,
      "width",
      ((graph.getEdgeAttribute(edge, "width") - minDegreeEdge) /
        (maxDegree - maxDegreeEdge)) *
        (MAX_EDGE_SIZE - MIN_EDGE_SIZE) +
        MIN_EDGE_SIZE
    );
  });

  renderer = new Sigma(graph, container, {
    enableEdgeClickEvents: true,
    allowInvalidContainer: true,
  });

  renderer.on("clickNode", ({ node }) => selectNode(node));

  renderer.setSetting("nodeReducer", (node, data) => {
    const res = { ...data };

    if (
      state.selectedNeighbors &&
      !state.selectedNeighbors.has(node) &&
      state.selectedNode !== node
    ) {
      res.hidden = true;
    }

    if (state.selectedNode === node) {
      res.highlighted = true;
    }

    return res;
  });

  renderer.setSetting("edgeReducer", (edge, data) => {
    const res = { ...data };

    if (state.selectedNode && !graph.hasExtremity(edge, state.selectedNode)) {
      res.hidden = true;
    }

    return res;
  });

  $(".returntext").click(() => {
    selectNode(null);
  });
}

/* SELECTION DE L ETUDE EN ACCUEIL */
$(document).ready(function () {
  try {
    var list = document.querySelectorAll(".accueil-bot__studies__list > li");
    var etudes = document.querySelectorAll(".etude-panel");
    for (let i = 0; i < list.length; i++) {
      list[i].addEventListener("click", function () {
        if (!this.classList.contains("picto-studie--selected")) {
          list.forEach((item) => {
            item.classList.remove("picto-studie--selected");
            $(item.getAttribute("data-target")).hide();
          });
          etudes.forEach((etude) => {
            etude.classList.add("etude-hidden");
          });
          this.classList.add("picto-studie--selected");

          if (this.children[0].classList.contains("crise")) {
            let crise = document.querySelector("#etude-crise");
            crise.classList.remove("etude-hidden");
          } else if (this.children[0].classList.contains("justice")) {
            let justice = document.querySelector("#etude-justice");
            justice.classList.remove("etude-hidden");
          } else if (this.children[0].classList.contains("tendance")) {
            let tendance = document.querySelector("#etude-tendance");
            tendance.classList.remove("etude-hidden");
            if (renderer !== undefined) {
              renderer.refresh();
            }
          }
        }
      });
    }
  } catch (error) {}

  if ($(".slick-accueil") && $(".slick-accueil").length) {
    $(".slick-accueil").slick({
      infinite: true,
      speed: 300,
      autoplay: true,
      autoplaySpeed: 2000,
      variableWidth: true,
      responsive: [
        {
          breakpoint: 1280,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 3,
            slidesToScroll: 3,
          },
        },
        {
          breakpoint: 990,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 2,
            slidesToScroll: 2,
          },
        },
        {
          breakpoint: 700,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 1,
            slidesToScroll: 1,
          },
        },
      ],
    });
  }

  if ($(".slick-tutoriel") && $(".slick-tutoriel").length) {
    $(".slick-tutoriel").slick({
      infinite: false,
      speed: 300,
      autoplay: false,
      autoplaySpeed: 2000,
      variableWidth: true,
      slidesToShow: 1,
      slidesToScroll: 1,
      responsive: [
        {
          breakpoint: 1280,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToScroll: 4,
          },
        },
        {
          breakpoint: 990,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 2,
            slidesToScroll: 2,
          },
        },
        {
          breakpoint: 700,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 1,
            slidesToScroll: 1,
          },
        },
      ],
    });
    let allSteps = document.getElementById("btn-skip");
    let lastStep = document.getElementById("btn-end-of-tuto");
    var skipTuto = document.getElementById("modal--tuto--skip");
    $(".slick-tutoriel").on("afterChange", function (e, s, currentSlideIndex) {
      if (currentSlideIndex === 3) {
        allSteps.style.display = "none";
        lastStep.style.display = "block";
        skipTuto.classList.add("btn-end-of-tuto");
      } else {
        allSteps.style.display = "block";
        lastStep.style.display = "none";
        skipTuto.classList.remove("btn-end-of-tuto");
      }
    });
  }

  if ($(".slick-publications") && $(".slick-publications").length) {
    $(".slick-publications").slick({
      infinite: false,
      speed: 300,
      // autoplay: true,
      // autoplaySpeed: 2000,
      slidesToShow: 3,
      slidesToScroll: 1,
      responsive: [
        {
          breakpoint: 990,
          settings: {
            autoplaySpeed: 3500,
            slidesToShow: 2,
            slidesToScroll: 2,
          },
        },
        {
          breakpoint: 700,
          settings: {
            autoplaySpeed: 3500,
            slidesToShow: 1,
            slidesToScroll: 1,
          },
        },
      ],
    });
  }

  /*Modal d'inscription*/
  // Get the modal
  var modal = document.getElementById("modal--plan");
  var tutoModal = document.getElementById("modal--tuto");

  if (modal) {
    const axiosClient = axios.create({
      headers: {
        "Content-type": "application/json",
      },
      withCredentials: true,
    });
    axiosClient.get("/auth/registration?").then((res) => {
      axiosClient
        .get("/auth/registration?flow=" + res.data.id)
        .then((resHtml) => {
          modal.innerHTML = resHtml.data;
        });
    });
  }

  // Get the button that opens the modal
  var btnPremium = document.getElementById("premium--button");
  var btnExpert = document.getElementById("expert--button");
  var btnTuto = document.getElementById("tuto--button");

  // Get the <span> element that closes the modal
  var close = document.getElementById("modal--plan--close");
  var closeTuto = document.getElementById("modal--tuto--close");
  var skipTuto = document.getElementById("modal--tuto--skip");
  // When the user clicks on the button, open the modal
  if (btnTuto)
    btnTuto.onclick = function () {
      tutoModal.style.display = "block";
    };

  if (btnPremium)
    btnPremium.onclick = function () {
      modal.style.display = "block";
    };

  if (btnExpert)
    btnExpert.onclick = function () {
      modal.style.display = "block";
    };

  if (close)
    close.onclick = function () {
      modal.style.display = "none";
    };

  if (closeTuto)
    closeTuto.onclick = function () {
      tutoModal.style.display = "none";
    };

  if (skipTuto)
    skipTuto.onclick = function () {
      tutoModal.style.display = "none";
    };

  if (close || closeTuto || skipTuto)
    // When the user clicks anywhere outside of the modal, close it
    window.onclick = function (event) {
      if (event.target == modal) {
        modal.style.display = "none";
      }
    };
});
