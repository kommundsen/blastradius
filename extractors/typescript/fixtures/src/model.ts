// Types other fixture modules import, plus an inheritance pair.
export interface Entity {
  id: string;
}

export class Order implements Entity {
  id = '';
}

export enum Status {
  Open,
  Closed,
}
