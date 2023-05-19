// This file is responsible for booting the elm application with as little code as
// possible. There should not be any specific business logic implemented here.

type Environment = {
  api: string,
  root: string,
  loginUrl: string,
  logoutUrl: string,
  version: string,
  release: boolean,
  localization: Array<[string, string]>,
};

const REPO_URL = 'https://github.com/dadleyy/orient-beetle';

// TODO: incorporate this into the project somewhere else. Would probably require some additional
// build tooling. Skimping on that for now.
const LOCALIZATION = [
  [
    'login_page', 
    `<h2>Orient Beetle</h2>

    <div class="my-4">
      <p>This project is meant to explore the esp32 microcontroller, rendering to a tft
      display, and managing the content of that rendering by some process running in the
      "cloud".</p>

      <p>All source code is available on <a href="${REPO_URL}">github</a>,
      including the hardware required and links to vendors where purchase may be
      executed.</p>
    </div>

    <h3>Latest Update:</h3>
    <div style="position: relative; width: 100%; height: 100%; max-height: 600px">
      <iframe
        height="100%"
        width="100%"
        src="https://www.youtube.com/embed/SwGfOSLR5tA"
        title="YouTube video player"
        frameborder="0"
        allow="accelerometer; autoplay; clipboard-write; encrypted-media; gyroscope; picture-in-picture"
        allowfullscreen
      ></iframe>
    </div>
    `,
  ],
];

(function () {
  function parse(input: string): Environment | undefined {
    try {
      return JSON.parse(input);
    } catch (error) {
      console.error(`unable to parse - ${error}`);
      return void 0;
    }
  }

  function boot(): void {
    const config = document.querySelector('meta[name=environment]');
    const value = config ? config.getAttribute('value') : null;
    const environment = typeof value === 'string' ? parse(value) : null;

    if (!environment) {
      console.warn('unable to find environment container');
      return;
    }

    console.info(`booting application with ${JSON.stringify(environment)}`);
    const flags = { ...environment, localization: LOCALIZATION };
    Elm.Main.init({ flags });
  }

  window.addEventListener('DOMContentLoaded', boot);
})();
