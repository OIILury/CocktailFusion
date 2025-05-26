var gulp = require("gulp");
var sass = require("gulp-sass")(require("sass"));
var sourcemaps = require("gulp-sourcemaps");
var postcss = require("gulp-postcss");
var autoprefixer = require("autoprefixer");
var notify = require("gulp-notify");
var plumber = require("gulp-plumber");

var input = "templates/static/scss/**/*.scss";
var output = function (f) {
  return f.base;
};
var sassOptions = {
  errLogToConsole: false,
  outputStyle: "expanded",
};

gulp.task("sass", function () {
  return (
    gulp
      .src(input)
      .pipe(
        plumber({ errorHandler: notify.onError("Error: <%= error.message %>") })
      )
      // .pipe(notify())
      .pipe(sourcemaps.init())
      .pipe(sass(sassOptions))
      .pipe(
        postcss([
          autoprefixer({
            // grid: true,
            cascade: false,
          }),
        ])
      )
      .pipe(sourcemaps.write("."))
      .pipe(gulp.dest("templates/static/css"))
  );
});

gulp.task("watch-sass", function () {
  return (
    gulp
      // Watch the input folder for change,
      // and run `sass` task when something happens
      .watch(input, gulp.series("sass"))
      // When there is a change,
      // log a message in the console
      .on("change", function (event) {
        console.log("File " + event + ", running tasks...");
      })
  );
});

gulp.task("watch", gulp.series("sass", "watch-sass"));
