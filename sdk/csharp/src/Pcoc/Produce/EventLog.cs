namespace Pcoc.Produce;

/// §13.5
[Spec("§13.5", "Only the live profile writes this log.")]
public sealed class EventLog
{
    public static bool SwapPostKeys;
    public static bool WriteOnTest;

    private readonly string? _path;
    private readonly bool _live;
    private readonly System.Collections.Generic.List<string> _records = new();

    [Spec("§13.5", "Only the live profile writes this log.")]
    public EventLog(string? objectPath, bool live)
    {
        _live = live;
        if (live && objectPath != null)
        {
            _path = objectPath + ".events";
            System.IO.File.WriteAllText(_path, "");
        }
        else if (WriteOnTest && objectPath != null)
        {
            _path = objectPath + ".events";
            System.IO.File.WriteAllText(_path, "");
        }
    }

    public System.Collections.Generic.IReadOnlyList<string> Records => _records;

    [Spec("§13.5", "Posted: event, stage, waypoint, attempt, posted, succeeded, dropped, then status only when Bari answered.")]
    public void Posted(int stage, byte[] waypoint, int attempt, bool posted, bool succeeded, bool dropped, int? status)
    {
        if (!_live && !WriteOnTest)
        {
            return;
        }
        var wp = Bytes.Hex(waypoint);
        string json;
        if (SwapPostKeys)
        {
            json = "{\"stage\":" + stage.ToString(System.Globalization.CultureInfo.InvariantCulture)
                + ",\"event\":\"hike.produce.posted\",\"waypoint\":\"" + wp + "\",\"attempt\":" + attempt.ToString(System.Globalization.CultureInfo.InvariantCulture)
                + ",\"posted\":" + (posted ? "true" : "false")
                + ",\"succeeded\":" + (succeeded ? "true" : "false")
                + ",\"dropped\":" + (dropped ? "true" : "false");
        }
        else
        {
            json = "{\"event\":\"hike.produce.posted\",\"stage\":" + stage.ToString(System.Globalization.CultureInfo.InvariantCulture)
                + ",\"waypoint\":\"" + wp + "\",\"attempt\":" + attempt.ToString(System.Globalization.CultureInfo.InvariantCulture)
                + ",\"posted\":" + (posted ? "true" : "false")
                + ",\"succeeded\":" + (succeeded ? "true" : "false")
                + ",\"dropped\":" + (dropped ? "true" : "false");
        }
        if (status.HasValue)
        {
            json += ",\"status\":" + status.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
        json += "}";
        Append(json);
    }

    [Spec("§13.5", "Alert: event, alert, stage, then height only when there is one.")]
    public void Alert(string name, int stage, ulong? height)
    {
        if (!_live && !WriteOnTest)
        {
            return;
        }
        var json = "{\"event\":\"hike.produce.alert\",\"alert\":\"" + name + "\",\"stage\":" + stage.ToString(System.Globalization.CultureInfo.InvariantCulture);
        if (height.HasValue)
        {
            json += ",\"height\":" + height.Value.ToString(System.Globalization.CultureInfo.InvariantCulture);
        }
        json += "}";
        Append(json);
    }

    [Spec("§13.5", "Every append is an asynchronous event (Rule A6).")]
    public void Append(string record)
    {
        _records.Add(record);
        if (_path == null)
        {
            return;
        }
        try
        {
            System.IO.File.AppendAllText(_path, record + "\n");
        }
        catch (System.Exception)
        {
        }
    }
}
