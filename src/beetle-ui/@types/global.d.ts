type BeetleFlags = {
};

type ElmInitialization = {
  node?: HTMLElement | null,
  flags?: BeetleFlags,
};

type ElmMain = {
  init: (opts: ElmInitialization) => void;
};

type ElmRuntime = {
  Main: ElmMain,
};

const Elm: ElmRuntime;
