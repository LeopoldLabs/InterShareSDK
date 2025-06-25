using InterShareSdk;

namespace Tests;

public class DiscoveryTests : IDiscoveryDelegate
{
    [Test]
    public async Task Setup()
    {
        var discovery = new Discovery(this);
        discovery.Start();

        while (true)
        {
            await Task.Delay(100);
        }
    }

    public void DeviceAdded(Device value)
    {
        Console.WriteLine($"Device discovered: {value}");
    }

    public void DeviceRemoved(string deviceId)
    {
        throw new NotImplementedException();
    }
}
