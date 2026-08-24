using Acme.Billing;
using Newtonsoft.Json;   // external: rolls up to dep.Newtonsoft
using System.Text;       // the BCL: never rolled up

namespace Acme.Shipping
{
    public interface ITracked
    {
        string TrackingNumber { get; }
    }

    public enum Carrier
    {
        Post,
        Courier,
    }

    public class Shipment : ITracked
    {
        public string TrackingNumber { get; set; } = "";
        public Carrier Carrier { get; set; }
        public Invoice? Invoice { get; set; }

        private class Label // nested: folds into Shipment, never an element
        {
        }
    }
}
