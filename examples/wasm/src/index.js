(async () => {
  const module = await import("../pkg");

  window.parse = module.parse;

  const input = document.getElementById("input");
  const form = document.getElementById("form");
  const output = document.getElementById("output");

  form.addEventListener("submit", event => {
    event.preventDefault();

    try {
        const value = input.value;
        input.value = "";
        window.parse(value);
    } catch (error) {
        output.innerHTML = error + ".\nSee dev tools console for stack trace / more information.";
        throw error;
    }
  });
})();
