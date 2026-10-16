package fixture;

public class Address implements java.io.Serializable {
    private static final long serialVersionUID = 1L;
    private String street;
    private int zip;

    public String getStreet() { return street; }
    public void setStreet(String street) { this.street = street; }
    public int getZip() { return zip; }
    public void setZip(int zip) { this.zip = zip; }
}
