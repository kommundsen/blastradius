// A global using: invisible to any single-file syntax pass, decisive to the
// compiler. Consumer.cs has no using of its own, so only semantic mode can
// know its `Widget` is Alpha's.
global using Alpha;
