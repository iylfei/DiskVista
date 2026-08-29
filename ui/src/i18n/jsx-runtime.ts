import {
  Fragment,
  jsx as reactJsx,
  jsxs as reactJsxs,
} from "react/jsx-runtime";
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

export function jsx(
  type: Parameters<typeof reactJsx>[0],
  props: Parameters<typeof reactJsx>[1],
  key?: Parameters<typeof reactJsx>[2],
) {
  return reactJsx(
    type,
    localizedProps(type, props as Record<string, unknown> | null),
    key,
  );
}

export function jsxs(
  type: Parameters<typeof reactJsxs>[0],
  props: Parameters<typeof reactJsxs>[1],
  key?: Parameters<typeof reactJsxs>[2],
) {
  return reactJsxs(
    type,
    localizedProps(type, props as Record<string, unknown> | null),
    key,
  );
}
