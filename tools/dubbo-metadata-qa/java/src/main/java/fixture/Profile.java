package fixture;

import java.util.List;
import java.util.Map;

public class Profile implements java.io.Serializable {
    private static final long serialVersionUID = 1L;
    private String name;
    private int age;
    private Address address;
    private Tier tier;
    private List<String> tags;
    private List<Address> offices;
    private Map<String, Address> addresses;

    public String getName() { return name; }
    public void setName(String name) { this.name = name; }
    public int getAge() { return age; }
    public void setAge(int age) { this.age = age; }
    public Address getAddress() { return address; }
    public void setAddress(Address address) { this.address = address; }
    public Tier getTier() { return tier; }
    public void setTier(Tier tier) { this.tier = tier; }
    public List<String> getTags() { return tags; }
    public void setTags(List<String> tags) { this.tags = tags; }
    public List<Address> getOffices() { return offices; }
    public void setOffices(List<Address> offices) { this.offices = offices; }
    public Map<String, Address> getAddresses() { return addresses; }
    public void setAddresses(Map<String, Address> addresses) { this.addresses = addresses; }
}
