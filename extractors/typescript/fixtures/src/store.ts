// Relative import (stays internal) alongside external packages: a bare
// specifier that is not installed here, a scoped one, a subpath, and a
// `node:` builtin that is the stdlib rather than a dependency.
import { Order } from './model.js';
import leftPad from 'left-pad';
import { render } from '@scope/widgets/render';
import { readFile } from 'node:fs/promises';

export class Store {
  orders: Order[] = [];
}
