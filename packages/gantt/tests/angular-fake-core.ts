// A stand-in for `@angular/core` (gantt#1875), for the Angular wrapper spec: the few names `src/angular.ts` imports, and a record of what its decorators say in `globalThis.declared`.
export class ElementRef {}
export class ErrorHandler {}
export class NgZone {}
export class EventEmitter<T> {
  private readonly handlers = new Set<(value: T) => void>();
  get observed(): boolean {
    return this.handlers.size > 0;
  }
  subscribe(handler: (value: T) => void): void {
    this.handlers.add(handler);
  }
  emit(value: T): void {
    this.handlers.forEach((handler) => handler(value));
  }
}
const declared = (): Record<string, any> => ((globalThis as { declared?: Record<string, any> }).declared ??= {});
export const Input = () => (_proto: object, key: string) => void ((declared().inputs ??= {})[key] = key);
export const Output = () => (_proto: object, key: string) => void ((declared().outputs ??= {})[key] = key);
export const Component = (meta: object) => () => void (declared().component = { ...meta, inputs: declared().inputs, outputs: declared().outputs });
