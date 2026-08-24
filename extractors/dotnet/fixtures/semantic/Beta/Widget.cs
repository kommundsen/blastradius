namespace Beta;

// Same simple name as Alpha.Widget — this is what makes name-based
// resolution ambiguous, and what the semantic pass disambiguates.
public class Widget
{
    public int Size { get; set; }
}
