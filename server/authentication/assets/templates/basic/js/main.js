window.addEventListener("DOMContentLoaded", (event) => {
  /* LEGENDE PAGE GRAPHIQUE*/
  /* Récupère les hauteurs vers une variable css pour déduire de 100vh */
  try {
    var staticHeightLegend =
    document.querySelector('.header-site').offsetHeight +
    document.querySelector('.aside-graphique__top').offsetHeight +
    document.querySelector('.footer-site').offsetHeight;
    document.documentElement.style.setProperty('--staticHeightLegend', staticHeightLegend + 'px');
  } catch (error) {
  }

  /* NOUVEAU PROJET */
  /* OUVERTURE / FERMETURE DU PANIER SELECTION */
  if (document.querySelector('.cart__detail--show')) {
    document.querySelector('.cart__detail--show').addEventListener('click', function() {
      document.querySelector('.newProject').classList.add('cart--open');
    });
    document.querySelector('.cart__detail--hide').addEventListener('click', function() {
      document.querySelector('.newProject').classList.remove('cart--open');
    });
  }

  /* OUVERTURE / FERMETURE DU MENU DANS LA SIDEBAR GAUCHE PROJET */
  document.onclick = function(e) {
    if (document.querySelector('.aside-project-extra')) {
      var extraMenuButton = document.querySelector('.aside-project-extra');
      var extraMenu = document.querySelector('.aside-project-extra-nav');

      if (e.target === extraMenuButton) {
        if (extraMenu.classList.contains('open')) {
          extraMenu.classList.remove('open');
        } else {
          extraMenu.classList.add('open');
        }

      } else {
        extraMenu.classList.remove('open');
      }
    }
  }
});

/* AU CHARGEMENT + RESIZE */
window.addEventListener("load", onLoadFunction);

function onLoadFunction(e) {
  onResizeFunction();

  window.addEventListener("resize", onResizeFunction);
}

function onResizeFunction(e) {
  /* HAUTEUR HEADER POUR COMPENSER LE FIXED */
  if (document.querySelector('.header-site')) {
    var height = document.querySelector('.header-site').offsetHeight;
    document.querySelector('.main-site').style.marginTop = height + "px";
  }
}
/* AU CHARGEMENT + RESIZE FIN*/


/* CAROUSEL SLICK */
$(document).ready(function () {
  if ($('.slick-connection') && $('.slick-connection').length) {
    $('.slick-connection').slick({
      infinite: true,
      speed: 300,
      autoplay: true,
      autoplaySpeed: 2000,
      slidesToShow: 1,
      variableWidth: true,
      responsive: [
        {
          breakpoint: 1280,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 3,
            slidesToScroll: 3
          }
        },
        {
          breakpoint: 990,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 2,
            slidesToScroll: 2
          }
        },
        {
          breakpoint: 700,
          settings: {
            autoplaySpeed: 3500,
            variableWidth: false,
            slidesToShow: 1,
            slidesToScroll: 1
          }
        }
      ]
    });
  }

  if ($('.slick-publications') && $('.slick-publications').length) {
    $('.slick-publications').slick({
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
            slidesToScroll: 2
          }
        },
        {
          breakpoint: 700,
          settings: {
            autoplaySpeed: 3500,
            slidesToShow: 1,
            slidesToScroll: 1
          }
        }
      ]
    });
  }
});
