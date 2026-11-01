// Minimale canvas-mock voor jsdom: elke 2d-methode is een no-op die zijn naam registreert,
// zodat tests kunnen nagaan dát er getekend is. Importeer dit bestand bovenaan een test.
type Ctx = CanvasRenderingContext2D & { calls: string[] };

const contexts = new WeakMap<HTMLCanvasElement, Ctx>();

function makeContext(canvas: HTMLCanvasElement): Ctx {
  const state: Record<string | symbol, unknown> = {
    canvas,
    calls: [] as string[],
    fillStyle: "#000",
    strokeStyle: "#000",
    lineWidth: 1,
    font: "10px sans-serif",
    globalAlpha: 1,
    shadowColor: "transparent",
    shadowBlur: 0,
    shadowOffsetX: 0,
    shadowOffsetY: 0,
    imageSmoothingEnabled: true,
    textBaseline: "alphabetic",
    textAlign: "start",
  };
  return new Proxy(state, {
    get(target, prop) {
      if (prop in target) return target[prop];
      if (prop === "measureText") return () => ({ width: 0 });
      if (prop === "getImageData") return () => ({ data: new Uint8ClampedArray(4), width: 1, height: 1 });
      if (prop === "createLinearGradient" || prop === "createRadialGradient" || prop === "createPattern") {
        return () => ({ addColorStop: () => undefined });
      }
      return (..._args: unknown[]) => {
        (target.calls as string[]).push(String(prop));
      };
    },
    set(target, prop, value) {
      target[prop] = value;
      return true;
    },
  }) as unknown as Ctx;
}

HTMLCanvasElement.prototype.getContext = function getContext(this: HTMLCanvasElement, type: string) {
  if (type !== "2d") return null;
  let ctx = contexts.get(this);
  if (!ctx) {
    ctx = makeContext(this);
    contexts.set(this, ctx);
  }
  return ctx;
} as typeof HTMLCanvasElement.prototype.getContext;

HTMLCanvasElement.prototype.toDataURL = () => "data:image/png;base64,";

/** De geregistreerde methodenamen van een canvas, voor asserties. */
export function callsOf(canvas: HTMLCanvasElement): string[] {
  return (canvas.getContext("2d") as Ctx).calls;
}
