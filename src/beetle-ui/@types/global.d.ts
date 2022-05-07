// esint-disable import/extensions

// This should match the `Flags` type defined in `Main.elm`
type BeetleFlags = {
  api: string,
  root: string,
  version: string,
  release: boolean,
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

declare const Elm: ElmRuntime;
