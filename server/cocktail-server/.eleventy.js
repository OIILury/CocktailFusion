module.exports = function (eleventyConfig) {
  eleventyConfig.setUseGitIgnore(false);
  eleventyConfig.addPassthroughCopy("templates/static");
  return {
    passthroughFileCopy: true,
  };
};
