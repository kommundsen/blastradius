namespace Gamma;

public class Consumer
{
    // Resolves to Alpha.Widget via the project's global using — across a
    // project reference. Syntax mode sees two candidate `Widget`s and drops
    // the edge rather than guessing.
    public Widget? Held { get; set; }
}
