namespace Acme.Billing;

public record InvoiceLine(string Sku, decimal Amount);

public partial class Invoice
{
    public List<InvoiceLine> Lines { get; } = new();
}
