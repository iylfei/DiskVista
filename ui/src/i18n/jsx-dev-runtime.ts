import { Fragment, jsxDEV as reactJsxDev } from "react/jsx-dev-runtime";
import type { JSX as ReactJSX } from "react";
import { localizedProps } from "./translate";

export { Fragment };

export namespace JSX {
  export type Element = ReactJSX.Element;
  export type ElementType = ReactJSX.ElementType;
  export interface ElementClass extends ReactJSX.ElementClass {}
  export interface ElementAttributesProperty
    extends ReactJSX.ElementAttributesProperty {}
  export interface ElementChildrenAttribute
    extends ReactJSX.ElementChildrenAttribute {}
  export interface IntrinsicAttributes extends ReactJSX.IntrinsicAttributes {}
  export interface IntrinsicClassAttributes<T>
    extends ReactJSX.IntrinsicClassAttributes<T> {}
  export interface IntrinsicElements extends ReactJSX.IntrinsicElements {}
  export type LibraryManagedAttributes<C, P> =
    ReactJSX.LibraryManagedAttributes<C, P>;
}

export function jsxDEV(
  type: Parameters<typeof reactJsxDev>[0],
  props: Parameters<typeof reactJsxDev>[1],
  key: Parameters<typeof reactJsxDev>[2],
  isStaticChildren: Parameters<typeof reactJsxDev>[3],
  source: Parameters<typeof reactJsxDev>[4],
  self: Parameters<typeof reactJsxDev>[5],
) {
  return reactJsxDev(
    type,
    localizedProps(type, props as Record<string, unknown> | null),
    key,
    isStaticChildren,
    source,
    self,
  );
}
