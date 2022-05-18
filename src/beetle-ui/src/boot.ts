// This file is responsible for booting the elm application with as little code as
// possible. There should not be any specific business logic implemented here.

type Environment = {
  api: string,
  root: string,
  loginUrl: string,
  version: string,
  release: boolean,
};

(function() {
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
    const flags = { ...environment };
    Elm.Main.init({ flags });
  }

  window.addEventListener('DOMContentLoaded', boot);
})();
