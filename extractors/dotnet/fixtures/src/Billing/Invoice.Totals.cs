namespace Acme.Billing;

public partial class Invoice
{
    public decimal Total => Lines.Sum(l => l.Amount);
}
