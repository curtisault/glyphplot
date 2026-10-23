// The Elm app as main.ts imports it: vite.config.ts's Elm plugin compiles
// site/src/Main.elm with `elm make` and exports its `Elm` object.

/** A port from Elm: main.ts subscribes to it. */
interface ElmPortOut<T> {
  subscribe(f: (value: T) => void): void;
}

/** A port into Elm: main.ts sends on it. */
interface ElmPortIn<T> {
  send(value: T): void;
}

/** site/src/Ports.elm's ports. */
interface SitePorts {
  setTheme: ElmPortOut<string>;
  systemTheme: ElmPortIn<string>;
  plotCall: ElmPortOut<{ id: string; method: string; args: unknown }>;
  plotReply: ElmPortIn<{ id: string; method: string; value: string }>;
  copy: ElmPortOut<{ id: string; text: string }>;
  copied: ElmPortIn<{ id: string; ok: boolean }>;
  requestSource: ElmPortOut<string>;
  gotSource: ElmPortIn<{ slug: string; text: string | null }>;
}

declare module "*.elm" {
  export const Elm: {
    Main: {
      init(options: { node?: HTMLElement; flags: unknown }): { ports: SitePorts };
    };
  };
}
